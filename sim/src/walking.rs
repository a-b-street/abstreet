use crate::instrument::capture_backtrace;
use crate::intersections::{IntersectionSimState, Request};
use crate::parking::ParkingSimState;
use crate::trips::TripManager;
use crate::view::{AgentView, WorldView};
use crate::{
    AgentID, Distance, DrawPedestrianInput, Event, ParkingSpot, PedestrianID, Speed, Tick, Time,
    TripID, TIMESTEP,
};
use abstutil;
use abstutil::{deserialize_multimap, serialize_multimap, Error};
use derivative::Derivative;
use dimensioned::si;
use geom::{Line, Pt2D};
use map_model::{
    BuildingID, BusStopID, IntersectionID, LaneID, LaneType, Map, Path, PathStep, Position, Trace,
    Traversable, TurnID,
};
use multimap::MultiMap;
use serde_derive::{Deserialize, Serialize};
use std;
use std::collections::{BTreeMap, HashSet};

// TODO tune these!
// TODO make it vary, after we can easily serialize these
// TODO temporarily very high to debug peds faster
const SPEED: Speed = si::MeterPerSecond {
    value_unsafe: 3.9,
    _marker: std::marker::PhantomData,
};

const TIME_TO_PREPARE_BIKE: Time = si::Second {
    value_unsafe: 15.0,
    _marker: std::marker::PhantomData,
};

// A pedestrian can start from a parking spot (after driving and parking) or at a building.
// A pedestrian can end at a parking spot (to start driving) or at a building.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SidewalkSpot {
    connection: SidewalkPOI,
    pub sidewalk_pos: Position,
}

impl SidewalkSpot {
    pub fn parking_spot(
        spot: ParkingSpot,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) -> SidewalkSpot {
        let sidewalk = map
            .find_closest_lane(spot.lane, vec![LaneType::Sidewalk])
            .unwrap();
        SidewalkSpot {
            connection: SidewalkPOI::ParkingSpot(spot),
            sidewalk_pos: parking_sim.spot_to_sidewalk_pos(spot, sidewalk, map),
        }
    }

    pub fn building(bldg: BuildingID, map: &Map) -> SidewalkSpot {
        let front_path = &map.get_b(bldg).front_path;
        SidewalkSpot {
            connection: SidewalkPOI::Building(bldg),
            sidewalk_pos: front_path.sidewalk,
        }
    }

    pub fn bike_rack(sidewalk_pos: Position, map: &Map) -> SidewalkSpot {
        assert!(map.get_l(sidewalk_pos.lane()).is_sidewalk());
        SidewalkSpot {
            connection: SidewalkPOI::BikeRack,
            sidewalk_pos,
        }
    }

    pub fn bus_stop(stop: BusStopID, map: &Map) -> SidewalkSpot {
        SidewalkSpot {
            sidewalk_pos: map.get_bs(stop).sidewalk_pos,
            connection: SidewalkPOI::BusStop(stop),
        }
    }

    pub fn start_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map.get_i(i).get_outgoing_lanes(map, LaneType::Sidewalk);
        if lanes.is_empty() {
            None
        } else {
            Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], 0.0 * si::M),
                connection: SidewalkPOI::Border(i),
            })
        }
    }

    pub fn end_at_border(i: IntersectionID, map: &Map) -> Option<SidewalkSpot> {
        let lanes = map.get_i(i).get_incoming_lanes(map, LaneType::Sidewalk);
        if lanes.is_empty() {
            None
        } else {
            Some(SidewalkSpot {
                sidewalk_pos: Position::new(lanes[0], map.get_l(lanes[0]).length()),
                connection: SidewalkPOI::Border(i),
            })
        }
    }
}

// Point of interest, that is
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum SidewalkPOI {
    ParkingSpot(ParkingSpot),
    Building(BuildingID),
    BusStop(BusStopID),
    Border(IntersectionID),
    BikeRack,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct CrossingFrontPath {
    bldg: BuildingID,
    // Measured from the building to the sidewalk
    dist_along: Distance,
    going_to_sidewalk: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct BikeParkingState {
    // False means departing
    is_parking: bool,
    started_at: Tick,
}

enum Action {
    StartParkedCar(ParkingSpot),
    WaitAtBusStop(BusStopID),
    StartCrossingPath(BuildingID),
    KeepCrossingPath,
    StartPreparingBike,
    KeepPreparingBike,
    Continue,
    TransitionToNextStep,
    WaitFor(TurnID),
    VanishAtBorder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Pedestrian {
    id: PedestrianID,
    trip: TripID,

    on: Traversable,
    dist_along: Distance,
    // A weird proxy for speed.
    moving: bool,

    // Head is the current step.
    path: Path,

    front_path: Option<CrossingFrontPath>,
    bike_parking: Option<BikeParkingState>,
    goal: SidewalkSpot,

    // If false, don't react() and step(). Waiting for a bus.
    active: bool,
}

impl Pedestrian {
    // Note this doesn't change the ped's state, and it observes a fixed view of the world!
    // TODO Quite similar to car's state and logic! Maybe refactor. Following paths, same four
    // actions, same transitions between turns and lanes...
    fn react(&self, map: &Map, intersections: &IntersectionSimState) -> Action {
        if self.front_path.is_some() {
            return Action::KeepCrossingPath;
        }
        if self.bike_parking.is_some() {
            return Action::KeepPreparingBike;
        }

        if self.path.is_last_step() {
            let goal_dist = self.goal.sidewalk_pos.dist_along();
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
                    SidewalkPOI::Border(_) => Action::VanishAtBorder,
                    SidewalkPOI::BikeRack => Action::StartPreparingBike,
                };
            }
            return Action::Continue;
        }

        {
            let contraflow = self.path.current_step().is_contraflow();
            if (!contraflow && self.dist_along < self.on.length(map))
                || (contraflow && self.dist_along > 0.0 * si::M)
            {
                return Action::Continue;
            }
        }

        if let PathStep::Turn(id) = self.path.next_step() {
            if !intersections.request_granted(Request::for_ped(self.id, id)) {
                return Action::WaitFor(id);
            }
        }

        Action::TransitionToNextStep
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
                    capture_backtrace("PedReachedBuilding");
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

        if self.path.current_step().is_contraflow() {
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

    fn step_transition(
        &mut self,
        events: &mut Vec<Event>,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<(), Error> {
        if let Traversable::Turn(t) = self.on {
            intersections.on_exit(Request::for_ped(self.id, t));
        }
        events.push(Event::AgentLeavesTraversable(
            AgentID::Pedestrian(self.id),
            self.on,
        ));

        // TODO could assert that it matches On
        self.path.shift();

        match self.path.current_step() {
            PathStep::Lane(id) => {
                self.on = Traversable::Lane(id);
                self.dist_along = 0.0 * si::M;
            }
            PathStep::ContraflowLane(id) => {
                self.on = Traversable::Lane(id);
                self.dist_along = map.get_l(id).length();
            }
            PathStep::Turn(t) => {
                self.on = Traversable::Turn(t);
                self.dist_along = 0.0 * si::M;
                intersections.on_enter(Request::for_ped(self.id, t))?;
            }
        }

        events.push(Event::AgentEntersTraversable(
            AgentID::Pedestrian(self.id),
            self.on,
        ));

        // TODO could calculate leftover (and deal with large timesteps, small
        // lanes)
        Ok(())
    }

    fn get_pos(&self, map: &Map, now: Tick) -> Pt2D {
        if let Some(ref fp) = self.front_path {
            map.get_b(fp.bldg).front_path.line.dist_along(fp.dist_along)
        } else if let Some(ref bp) = self.bike_parking {
            let sidewalk_pos = Position::new(self.on.as_lane(), self.dist_along);
            let street_pos = sidewalk_pos.equiv_pos(
                map.find_closest_lane(self.on.as_lane(), vec![LaneType::Driving, LaneType::Biking])
                    .unwrap(),
                map,
            );
            let line = Line::new(sidewalk_pos.pt(map), street_pos.pt(map));

            let progress: f64 =
                ((now - bp.started_at).as_time() / TIME_TO_PREPARE_BIKE).value_unsafe;
            assert!(progress >= 0.0 && progress <= 1.0);
            let ratio = if bp.is_parking {
                1.0 - progress
            } else {
                progress
            };

            line.dist_along(ratio * line.length())
        } else {
            self.on.dist_along(self.dist_along, map).0
        }
    }

    fn waiting_for_turn(&self) -> Option<TurnID> {
        if self.moving || self.path.is_last_step() {
            return None;
        }
        if let PathStep::Turn(id) = self.path.next_step() {
            return Some(id);
        }
        None
    }
}

#[derive(Serialize, Deserialize, Derivative, PartialEq)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    peds_per_bus_stop: MultiMap<BusStopID, PedestrianID>,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_traversable: MultiMap::new(),
            peds_per_bus_stop: MultiMap::new(),
        }
    }

    pub fn edit_remove_lane(&mut self, id: LaneID) {
        assert_eq!(
            self.peds_per_traversable.get_vec(&Traversable::Lane(id)),
            None
        );
    }

    pub fn edit_add_lane(&mut self, _id: LaneID) {
        // No-op
    }

    pub fn edit_remove_turn(&mut self, id: TurnID) {
        assert_eq!(
            self.peds_per_traversable.get_vec(&Traversable::Turn(id)),
            None
        );
    }

    pub fn edit_add_turn(&mut self, _id: TurnID) {
        // No-op
    }

    // Return all the pedestrians that have reached a parking spot and all the pedestrians that're
    // ready to start biking (and where they're starting from).
    pub fn step(
        &mut self,
        events: &mut Vec<Event>,
        delta_time: Time,
        now: Tick,
        map: &Map,
        intersections: &mut IntersectionSimState,
        trips: &mut TripManager,
        current_agent: &mut Option<AgentID>,
    ) -> Result<
        (
            Vec<(PedestrianID, ParkingSpot)>,
            Vec<(PedestrianID, Position)>,
        ),
        Error,
    > {
        // Could be concurrent, since this is deterministic.
        let mut requested_moves: Vec<(PedestrianID, Action)> = Vec::new();
        for p in self.peds.values() {
            if p.active {
                *current_agent = Some(AgentID::Pedestrian(p.id));
                requested_moves.push((p.id, p.react(map, intersections)));
            }
        }

        // In AORTA, there was a split here -- react vs step phase. We're still following the same
        // thing, but it might be slightly more clear to express it differently?

        let mut reached_parking = Vec::new();
        let mut ready_to_bike = Vec::new();

        // Apply moves. This can also be concurrent, since there are no possible conflicts.
        for (id, act) in &requested_moves {
            *current_agent = Some(AgentID::Pedestrian(*id));
            match *act {
                Action::StartCrossingPath(bldg) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.moving = true;
                    p.front_path = Some(CrossingFrontPath {
                        bldg,
                        dist_along: map.get_b(bldg).front_path.line.length(),
                        going_to_sidewalk: false,
                    });
                }
                Action::KeepCrossingPath => {
                    let done = {
                        let p = self.peds.get_mut(&id).unwrap();
                        p.moving = true;
                        p.step_cross_path(events, delta_time, map)
                    };
                    if done {
                        self.peds.remove(&id);
                        // TODO Should we return stuff to sim, or do the interaction here?
                        trips.ped_reached_building_or_border(*id, now);
                    }
                }
                Action::StartPreparingBike => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.moving = false;
                    p.bike_parking = Some(BikeParkingState {
                        is_parking: false,
                        started_at: now,
                    });
                }
                Action::KeepPreparingBike => {
                    let state = self.peds[&id].bike_parking.as_ref().unwrap().clone();
                    if (now - state.started_at).as_time() >= TIME_TO_PREPARE_BIKE {
                        if state.is_parking {
                            // Now they'll start walking somewhere
                            self.peds.get_mut(&id).unwrap().bike_parking = None;
                        } else {
                            let p = &self.peds[&id];
                            ready_to_bike.push((*id, Position::new(p.on.as_lane(), p.dist_along)));
                            self.peds.remove(&id);
                        }
                    }
                }
                Action::WaitAtBusStop(stop) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.active = false;
                    p.moving = false;
                    events.push(Event::PedReachedBusStop(*id, stop));
                    capture_backtrace("PedReachedBusStop");
                    self.peds_per_bus_stop.insert(stop, *id);
                }
                Action::StartParkedCar(ref spot) => {
                    self.peds.remove(&id);
                    reached_parking.push((*id, *spot));
                }
                Action::Continue => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.moving = true;
                    p.step_continue(delta_time, map);
                }
                Action::TransitionToNextStep => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.moving = true;
                    p.step_transition(events, map, intersections)?;
                }
                Action::WaitFor(turn) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.moving = false;
                    // Note this is idempotent and does NOT grant the request.
                    intersections.submit_request(Request::for_ped(*id, turn));
                }
                Action::VanishAtBorder => {
                    events.push(Event::AgentLeavesTraversable(
                        AgentID::Pedestrian(*id),
                        self.peds.get_mut(&id).unwrap().on,
                    ));
                    self.peds.remove(&id);
                    // TODO Should we return stuff to sim, or do the interaction here?
                    trips.ped_reached_building_or_border(*id, now);
                }
            }
        }
        *current_agent = None;

        self.peds_per_traversable.clear();
        for p in self.peds.values() {
            self.peds_per_traversable.insert(p.on, p.id);
        }

        Ok((reached_parking, ready_to_bike))
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        if let Some(ped) = self.peds.get(&id) {
            println!("{}", abstutil::to_json(ped));
        } else {
            println!("{} doesn't exist", id);
        }
    }

    pub fn get_draw_ped(
        &self,
        id: PedestrianID,
        map: &Map,
        now: Tick,
    ) -> Option<DrawPedestrianInput> {
        let ped = self.peds.get(&id)?;
        Some(DrawPedestrianInput {
            id,
            pos: ped.get_pos(map, now),
            waiting_for_turn: ped.waiting_for_turn(),
            preparing_bike: ped.bike_parking.is_some(),
            on: ped.on,
        })
    }

    pub fn get_draw_peds(&self, on: Traversable, map: &Map, now: Tick) -> Vec<DrawPedestrianInput> {
        let mut result = Vec::new();
        for id in self
            .peds_per_traversable
            .get_vec(&on)
            .unwrap_or(&Vec::new())
        {
            result.push(self.get_draw_ped(*id, map, now).unwrap());
        }
        result
    }

    pub fn get_all_draw_peds(&self, now: Tick, map: &Map) -> Vec<DrawPedestrianInput> {
        self.peds
            .values()
            .map(|ped| DrawPedestrianInput {
                id: ped.id,
                pos: ped.get_pos(map, now),
                waiting_for_turn: ped.waiting_for_turn(),
                preparing_bike: ped.bike_parking.is_some(),
                on: ped.on,
            })
            .collect()
    }

    pub fn seed_pedestrian(
        &mut self,
        events: &mut Vec<Event>,
        now: Tick,
        params: CreatePedestrian,
    ) {
        let start_lane = params.start.sidewalk_pos.lane();
        assert_eq!(
            params.path.current_step().as_traversable(),
            Traversable::Lane(start_lane)
        );
        assert_eq!(
            params.path.last_step().as_traversable(),
            Traversable::Lane(params.goal.sidewalk_pos.lane())
        );

        let front_path = match params.start.connection {
            SidewalkPOI::Building(id) => Some(CrossingFrontPath {
                bldg: id,
                dist_along: 0.0 * si::M,
                going_to_sidewalk: true,
            }),
            _ => None,
        };
        let bike_parking = match params.start.connection {
            SidewalkPOI::BikeRack => Some(BikeParkingState {
                is_parking: true,
                started_at: now,
            }),
            _ => None,
        };

        self.peds.insert(
            params.id,
            Pedestrian {
                id: params.id,
                trip: params.trip,
                path: params.path,
                on: Traversable::Lane(start_lane),
                dist_along: params.start.sidewalk_pos.dist_along(),
                front_path,
                bike_parking,
                goal: params.goal,
                moving: true,
                active: true,
            },
        );
        self.peds_per_traversable
            .insert(Traversable::Lane(start_lane), params.id);
        events.push(Event::AgentEntersTraversable(
            AgentID::Pedestrian(params.id),
            Traversable::Lane(start_lane),
        ));
    }

    pub fn populate_view(&self, view: &mut WorldView) {
        for p in self.peds.values() {
            let id = AgentID::Pedestrian(p.id);
            view.agents.insert(
                id,
                AgentView {
                    id,
                    debug: false,
                    on: p.on,
                    dist_along: p.dist_along,
                    speed: if p.moving { SPEED } else { 0.0 * si::MPS },
                    vehicle: None,
                },
            );
        }
    }

    pub fn get_active_and_waiting_count(&self) -> (usize, usize) {
        let waiting = self.peds.values().filter(|p| !p.moving).count();
        (waiting, self.peds.len())
    }

    pub fn is_done(&self) -> bool {
        self.peds.is_empty()
    }

    pub fn trace_route(&self, id: PedestrianID, map: &Map, dist_ahead: Distance) -> Option<Trace> {
        let p = self.peds.get(&id)?;
        p.path.trace(map, p.dist_along, dist_ahead)
    }

    pub fn get_path(&self, id: PedestrianID) -> Option<&Path> {
        let p = self.peds.get(&id)?;
        Some(&p.path)
    }

    pub fn get_peds_waiting_at_stop(&self, stop: BusStopID) -> Vec<PedestrianID> {
        // TODO ew, annoying multimap API and clone
        self.peds_per_bus_stop
            .get_vec(&stop)
            .unwrap_or(&Vec::new())
            .clone()
    }

    pub fn ped_joined_bus(&mut self, id: PedestrianID, stop: BusStopID) {
        self.peds.remove(&id);
        self.peds_per_bus_stop
            .get_vec_mut(&stop)
            .unwrap()
            .retain(|&p| p != id);
        self.peds_per_traversable
            .get_vec_mut(&Traversable::Lane(stop.sidewalk))
            .unwrap()
            .retain(|&p| p != id);
    }

    pub fn ped_tooltip(&self, id: PedestrianID) -> Vec<String> {
        let p = &self.peds[&id];
        vec![
            format!("{}", p.id),
            format!("{} lanes left in path", p.path.num_lanes()),
        ]
    }

    // TODO turns too
    pub fn count(&self, lanes: &HashSet<LaneID>) -> (usize, usize) {
        let mut moving_peds = 0;
        let mut stuck_peds = 0;

        for l in lanes {
            for ped in self
                .peds_per_traversable
                .get_vec(&Traversable::Lane(*l))
                .unwrap_or(&Vec::new())
            {
                let p = &self.peds[ped];
                if p.moving {
                    moving_peds += 1;
                } else {
                    stuck_peds += 1;
                }
            }
        }

        (moving_peds, stuck_peds)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct CreatePedestrian {
    pub id: PedestrianID,
    pub trip: TripID,
    pub start: SidewalkSpot,
    pub goal: SidewalkSpot,
    pub path: Path,
}
