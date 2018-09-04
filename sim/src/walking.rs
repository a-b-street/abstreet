use abstutil;
use abstutil::{deserialize_multimap, serialize_multimap};
use dimensioned::si;
use draw_ped::DrawPedestrian;
use geom::Pt2D;
use intersections::{AgentInfo, IntersectionSimState, Request};
use map_model::{BuildingID, BusStop, Lane, LaneID, Map, Turn, TurnID};
use multimap::MultiMap;
use parking::ParkingSimState;
use std;
use std::collections::{BTreeMap, VecDeque};
use {
    AgentID, Distance, Event, InvariantViolated, On, ParkingSpot, PedestrianID, Speed, Time,
    TIMESTEP,
};

// TODO tune these!
// TODO make it vary, after we can easily serialize these
// TODO temporarily very high to debug peds faster
const SPEED: Speed = si::MeterPerSecond {
    value_unsafe: 3.9,
    _marker: std::marker::PhantomData,
};

// A pedestrian can start from a parking spot (after driving and parking) or at a building.
// A pedestrian can end at a parking spot (to start driving) or at a building.
#[derive(Clone, Debug, Derivative, Serialize, Deserialize)]
#[derivative(PartialEq, Eq)]
pub struct SidewalkSpot {
    connection: SidewalkPOI,
    pub sidewalk: LaneID,
    #[derivative(PartialEq = "ignore")]
    dist_along: Distance,
}

impl SidewalkSpot {
    pub fn parking_spot(
        spot: ParkingSpot,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) -> SidewalkSpot {
        let sidewalk = map.get_parent(spot.lane).find_sidewalk(spot.lane).unwrap();
        let dist_along = parking_sim.dist_along_for_ped(spot);
        SidewalkSpot {
            connection: SidewalkPOI::ParkingSpot(spot),
            sidewalk,
            dist_along,
        }
    }

    pub fn building(bldg: BuildingID, map: &Map) -> SidewalkSpot {
        let front_path = &map.get_b(bldg).front_path;
        SidewalkSpot {
            connection: SidewalkPOI::Building(bldg),
            sidewalk: front_path.sidewalk,
            dist_along: front_path.dist_along_sidewalk,
        }
    }

    pub fn bus_stop(stop: BusStop, map: &Map) -> SidewalkSpot {
        SidewalkSpot {
            sidewalk: stop.sidewalk,
            dist_along: map.get_bus_stop(stop).dist_along,
            connection: SidewalkPOI::BusStop(stop),
        }
    }
}

// Point of interest, that is
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum SidewalkPOI {
    ParkingSpot(ParkingSpot),
    Building(BuildingID),
    BusStop(BusStop),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CrossingFrontPath {
    bldg: BuildingID,
    // Measured from the building to the sidewalk
    dist_along: Distance,
    going_to_sidewalk: bool,
}

enum Action {
    StartParkedCar(ParkingSpot),
    WaitAtBusStop(BusStop),
    StartCrossingPath(BuildingID),
    KeepCrossingPath,
    Continue,
    Goto(On),
    WaitFor(On),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pedestrian {
    id: PedestrianID,

    on: On,
    dist_along: Distance,
    // Traveling along the lane/turn in its original direction or not?
    contraflow: bool,

    // Head is the next lane
    path: VecDeque<LaneID>,
    waiting_for: Option<On>,

    front_path: Option<CrossingFrontPath>,
    goal: SidewalkSpot,

    // If false, don't react() and step(). Waiting for a bus.
    active: bool,
}

// TODO this is used for verifying sim state determinism, so it should actually check everything.
// the f64 prevents this from being derived.
impl PartialEq for Pedestrian {
    fn eq(&self, other: &Pedestrian) -> bool {
        self.id == other.id
    }
}
impl Eq for Pedestrian {}

impl Pedestrian {
    // Note this doesn't change the ped's state, and it observes a fixed view of the world!
    // TODO Quite similar to car's state and logic! Maybe refactor. Following paths, same four
    // actions, same transitions between turns and lanes...
    fn react(&self, map: &Map, intersections: &IntersectionSimState) -> Action {
        if self.front_path.is_some() {
            return Action::KeepCrossingPath;
        }

        if self.path.is_empty() {
            let goal_dist = self.goal.dist_along;
            // Since the walking model doesn't really have granular speed, just see if we're
            // reasonably close to the path.
            // Later distance will be non-negative, so don't attempt abs() or anything
            let dist_away = if self.dist_along > goal_dist {
                self.dist_along - goal_dist
            } else {
                goal_dist - self.dist_along
            };
            if dist_away <= 2.0 * SPEED * TIMESTEP {
                return match self.goal.connection {
                    SidewalkPOI::ParkingSpot(spot) => Action::StartParkedCar(spot),
                    SidewalkPOI::Building(id) => Action::StartCrossingPath(id),
                    SidewalkPOI::BusStop(stop) => Action::WaitAtBusStop(stop),
                };
            }
            return Action::Continue;
        }

        let desired_on: On = {
            if let Some(on) = self.waiting_for {
                on
            } else {
                if (!self.contraflow && self.dist_along < self.on.length(map))
                    || (self.contraflow && self.dist_along > 0.0 * si::M)
                {
                    return Action::Continue;
                }

                match self.on {
                    On::Lane(id) => On::Turn(self.choose_turn(id, map)),
                    On::Turn(id) => On::Lane(map.get_t(id).dst),
                }
            }
        };

        // Can we actually go there right now?
        let intersection_req_granted = match desired_on {
            // Already doing a turn, finish it!
            On::Lane(_) => true,
            On::Turn(id) => intersections.request_granted(Request::for_ped(self.id, id)),
        };
        if intersection_req_granted {
            Action::Goto(desired_on)
        } else {
            Action::WaitFor(desired_on)
        }
    }

    fn choose_turn(&self, from: LaneID, map: &Map) -> TurnID {
        assert!(self.waiting_for.is_none());
        for t in map.get_turns_from_lane(from) {
            if t.dst == self.path[0] {
                return t.id;
            }
        }
        panic!("No turn from {} to {}", from, self.path[0]);
    }

    // If true, then we're completely done!
    fn step_cross_path(&mut self, events: &mut Vec<Event>, delta_time: Time, map: &Map) -> bool {
        let new_dist = delta_time * SPEED;

        // TODO arguably a different direction would make this easier
        let done = if let Some(ref mut fp) = self.front_path {
            if fp.going_to_sidewalk {
                fp.dist_along += new_dist;
                fp.dist_along >= map.get_b(fp.bldg).front_path.line.length()
            } else {
                fp.dist_along -= new_dist;
                if fp.dist_along < 0.0 * si::M {
                    events.push(Event::PedReachedBuilding(self.id, fp.bldg));
                    return true;
                }
                false
            }
        } else {
            false
        };
        if done {
            self.front_path = None;
        }
        false
    }

    fn step_continue(&mut self, delta_time: Time, map: &Map) {
        let new_dist = delta_time * SPEED;

        if self.contraflow {
            self.dist_along -= new_dist;
            if self.dist_along < 0.0 * si::M {
                self.dist_along = 0.0 * si::M;
            }
        } else {
            self.dist_along += new_dist;
            let max_dist = self.on.length(map);
            if self.dist_along > max_dist {
                self.dist_along = max_dist;
            }
        }
    }

    fn step_goto(
        &mut self,
        events: &mut Vec<Event>,
        on: On,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<(), InvariantViolated> {
        let old_on = self.on.clone();
        if let On::Turn(t) = self.on {
            intersections.on_exit(Request::for_ped(self.id, t));
            assert_eq!(self.path[0], map.get_t(t).dst);
            self.path.pop_front();
        }
        events.push(Event::AgentLeavesTraversable(
            AgentID::Pedestrian(self.id),
            old_on,
        ));
        events.push(Event::AgentEntersTraversable(
            AgentID::Pedestrian(self.id),
            on,
        ));
        self.waiting_for = None;
        self.on = on;
        self.dist_along = 0.0 * si::M;
        self.contraflow = false;
        match self.on {
            On::Turn(t) => {
                intersections.on_enter(Request::for_ped(self.id, t))?;
            }
            On::Lane(l) => {
                // Which end of the sidewalk are we entering?
                // TODO are there cases where we should enter a new sidewalk and
                // immediately enter a different turn, instead of always going to the
                // other side of the sidealk? or are there enough turns to make that
                // unnecessary?
                let turn = map.get_t(old_on.as_turn());
                let lane = map.get_l(l);
                if turn.parent == lane.dst_i {
                    self.contraflow = true;
                    self.dist_along = lane.length();
                }
            }
        }

        // TODO could calculate leftover (and deal with large timesteps, small
        // lanes)
        Ok(())
    }

    fn get_pos(&self, map: &Map) -> Pt2D {
        if let Some(ref fp) = self.front_path {
            map.get_b(fp.bldg).front_path.line.dist_along(fp.dist_along)
        } else {
            self.on.dist_along(self.dist_along, map).0
        }
    }
}

#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    peds_per_sidewalk: MultiMap<LaneID, PedestrianID>,
    #[serde(serialize_with = "serialize_multimap")]
    #[serde(deserialize_with = "deserialize_multimap")]
    peds_per_turn: MultiMap<TurnID, PedestrianID>,
    #[serde(serialize_with = "serialize_multimap")]
    #[serde(deserialize_with = "deserialize_multimap")]
    peds_per_bus_stop: MultiMap<BusStop, PedestrianID>,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_sidewalk: MultiMap::new(),
            peds_per_turn: MultiMap::new(),
            peds_per_bus_stop: MultiMap::new(),
        }
    }

    pub fn edit_remove_lane(&mut self, id: LaneID) {
        assert_eq!(self.peds_per_sidewalk.get_vec(&id), None);
    }

    pub fn edit_add_lane(&mut self, _id: LaneID) {
        // No-op
    }

    pub fn edit_remove_turn(&mut self, id: TurnID) {
        assert_eq!(self.peds_per_turn.get_vec(&id), None);
    }

    pub fn edit_add_turn(&mut self, _id: TurnID) {
        // No-op
    }

    // Return all the pedestrians that have reached a parking spot.
    pub fn step(
        &mut self,
        events: &mut Vec<Event>,
        delta_time: Time,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<Vec<(PedestrianID, ParkingSpot)>, InvariantViolated> {
        // Could be concurrent, since this is deterministic.
        let mut requested_moves: Vec<(PedestrianID, Action)> = Vec::new();
        for p in self.peds.values() {
            if p.active {
                requested_moves.push((p.id, p.react(map, intersections)));
            }
        }

        // In AORTA, there was a split here -- react vs step phase. We're still following the same
        // thing, but it might be slightly more clear to express it differently?

        let mut results = Vec::new();

        // Apply moves. This can also be concurrent, since there are no possible conflicts.
        for (id, act) in &requested_moves {
            match *act {
                Action::KeepCrossingPath => {
                    if self.peds
                        .get_mut(&id)
                        .unwrap()
                        .step_cross_path(events, delta_time, map)
                    {
                        self.peds.remove(&id);
                    }
                }
                Action::WaitAtBusStop(stop) => {
                    self.peds.get_mut(&id).unwrap().active = false;
                    events.push(Event::PedReachedBusStop(*id, stop));
                    self.peds_per_bus_stop.insert(stop, *id);
                }
                Action::StartParkedCar(ref spot) => {
                    self.peds.remove(&id);
                    results.push((*id, *spot));
                }
                Action::StartCrossingPath(bldg) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.front_path = Some(CrossingFrontPath {
                        bldg,
                        dist_along: map.get_b(bldg).front_path.line.length(),
                        going_to_sidewalk: false,
                    });
                }
                Action::Continue => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.step_continue(delta_time, map);
                }
                Action::Goto(on) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.step_goto(events, on, map, intersections)?;
                }
                Action::WaitFor(on) => {
                    self.peds.get_mut(&id).unwrap().waiting_for = Some(on);
                    if let On::Turn(t) = on {
                        // Note this is idempotent and does NOT grant the request.
                        intersections.submit_request(Request::for_ped(*id, t))?;
                    }
                }
            }
        }

        // TODO could simplify this by only adjusting the sets we need above
        self.peds_per_sidewalk.clear();
        self.peds_per_turn.clear();
        for p in self.peds.values() {
            match p.on {
                On::Lane(id) => self.peds_per_sidewalk.insert(id, p.id),
                On::Turn(id) => self.peds_per_turn.insert(id, p.id),
            };
        }

        Ok(results)
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        if let Some(ped) = self.peds.get(&id) {
            println!("{}", abstutil::to_json(ped));
        } else {
            println!("{} doesn't exist", id);
        }
    }

    pub fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrian> {
        let ped = self.peds.get(&id)?;
        Some(DrawPedestrian::new(
            id,
            ped.get_pos(map),
            // TODO this isnt correct, but works right now because this is only called by warp
            None,
        ))
    }

    pub fn get_draw_peds_on_lane(&self, l: &Lane, map: &Map) -> Vec<DrawPedestrian> {
        let mut result = Vec::new();
        for id in self.peds_per_sidewalk.get_vec(&l.id).unwrap_or(&Vec::new()) {
            let ped = &self.peds[id];
            result.push(DrawPedestrian::new(
                *id,
                ped.get_pos(map),
                ped.waiting_for.map(|on| map.get_t(on.as_turn())),
            ));
        }
        result
    }

    pub fn get_draw_peds_on_turn(&self, t: &Turn) -> Vec<DrawPedestrian> {
        let mut result = Vec::new();
        for id in self.peds_per_turn.get_vec(&t.id).unwrap_or(&Vec::new()) {
            result.push(DrawPedestrian::new(
                *id,
                t.dist_along(self.peds[id].dist_along).0,
                None,
            ));
        }
        result
    }

    pub fn seed_pedestrian(
        &mut self,
        events: &mut Vec<Event>,
        id: PedestrianID,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        map: &Map,
        mut path: VecDeque<LaneID>,
    ) {
        let start_lane = path.pop_front().unwrap();
        assert_eq!(start_lane, start.sidewalk);
        if !path.is_empty() {
            assert_eq!(*path.back().unwrap(), goal.sidewalk);
        }
        let front_path = if let SidewalkPOI::Building(id) = start.connection {
            Some(CrossingFrontPath {
                bldg: id,
                dist_along: 0.0 * si::M,
                going_to_sidewalk: true,
            })
        } else {
            None
        };

        let contraflow = if path.is_empty() {
            start.dist_along > goal.dist_along
        } else {
            is_contraflow(map, start_lane, path[0])
        };
        self.peds.insert(
            id,
            Pedestrian {
                id,
                path,
                contraflow,
                on: On::Lane(start_lane),
                dist_along: start.dist_along,
                waiting_for: None,
                front_path,
                goal,
                active: true,
            },
        );
        self.peds_per_sidewalk.insert(start_lane, id);
        events.push(Event::AgentEntersTraversable(
            AgentID::Pedestrian(id),
            On::Lane(start_lane),
        ));
    }

    pub fn populate_info_for_intersections(&self, info: &mut AgentInfo) {
        for p in self.peds.values() {
            let id = AgentID::Pedestrian(p.id);
            info.speeds.insert(
                id,
                if p.waiting_for.is_some() {
                    0.0 * si::MPS
                } else {
                    SPEED
                },
            );
            info.leaders.insert(id);
        }
    }

    pub fn get_active_and_waiting_count(&self) -> (usize, usize) {
        let waiting = self.peds
            .values()
            .filter(|p| p.waiting_for.is_some())
            .count();
        (waiting, self.peds.len())
    }

    pub fn is_done(&self) -> bool {
        self.peds.is_empty()
    }

    pub fn get_current_route(&self, id: PedestrianID) -> Option<Vec<LaneID>> {
        self.peds
            .get(&id)
            .map(|p| p.path.iter().map(|id| *id).collect())
    }

    pub fn get_peds_waiting_at_stop(&self, stop: BusStop) -> Vec<PedestrianID> {
        // TODO ew, annoying multimap API and clone
        self.peds_per_bus_stop
            .get_vec(&stop)
            .unwrap_or(&Vec::new())
            .clone()
    }

    pub fn ped_joined_bus(&mut self, id: PedestrianID, stop: BusStop) {
        self.peds.remove(&id);
        self.peds_per_bus_stop
            .get_vec_mut(&stop)
            .unwrap()
            .retain(|&p| p != id);
        self.peds_per_sidewalk
            .get_vec_mut(&stop.sidewalk)
            .unwrap()
            .retain(|&p| p != id);
    }
}

fn is_contraflow(map: &Map, from: LaneID, to: LaneID) -> bool {
    map.get_l(from).dst_i != map.get_l(to).src_i
}
