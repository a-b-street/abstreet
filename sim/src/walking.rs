use abstutil;
use abstutil::{deserialize_multimap, serialize_multimap};
use dimensioned::si;
use draw_ped::DrawPedestrian;
use geom::Pt2D;
use intersections::{AgentInfo, IntersectionSimState, Request};
use map_model::{BuildingID, Lane, LaneID, Map, Turn, TurnID};
use multimap::MultiMap;
use std;
use std::collections::{BTreeMap, VecDeque};
use {AgentID, Distance, InvariantViolated, On, PedestrianID, Speed, Time, TIMESTEP};

// TODO tune these!
// TODO make it vary, after we can easily serialize these
// TODO temporarily very high to debug peds faster
const SPEED: Speed = si::MeterPerSecond {
    value_unsafe: 3.9,
    _marker: std::marker::PhantomData,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CrossingFrontPath {
    bldg: BuildingID,
    // Measured from the building to the sidewalk
    dist_along: Distance,
    going_to_sidewalk: bool,
}

enum Action {
    StartCrossingPath,
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
    goal: BuildingID,
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
            let goal_dist = map.get_b(self.goal).front_path.dist_along_sidewalk;
            // Since the walking model doesn't really have granular speed, just see if we're
            // reasonably close to the path.
            // Later distance will be non-negative, so don't attempt abs() or anything
            let dist_away = if self.dist_along > goal_dist {
                self.dist_along - goal_dist
            } else {
                goal_dist - self.dist_along
            };
            if dist_away <= 2.0 * SPEED * TIMESTEP {
                return Action::StartCrossingPath;
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
    fn step_cross_path(&mut self, delta_time: Time, map: &Map) -> bool {
        let new_dist = delta_time * SPEED;

        // TODO arguably a different direction would make this easier
        let done = if let Some(ref mut fp) = self.front_path {
            if fp.going_to_sidewalk {
                fp.dist_along += new_dist;
                fp.dist_along >= map.get_b(fp.bldg).front_path.line.length()
            } else {
                fp.dist_along -= new_dist;
                if fp.dist_along < 0.0 * si::M {
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
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_sidewalk: MultiMap::new(),
            peds_per_turn: MultiMap::new(),
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

    pub fn step(
        &mut self,
        delta_time: Time,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<(), InvariantViolated> {
        // Could be concurrent, since this is deterministic.
        let mut requested_moves: Vec<(PedestrianID, Action)> = Vec::new();
        for p in self.peds.values() {
            requested_moves.push((p.id, p.react(map, intersections)));
        }

        // In AORTA, there was a split here -- react vs step phase. We're still following the same
        // thing, but it might be slightly more clear to express it differently?

        // Apply moves. This can also be concurrent, since there are no possible conflicts.
        for (id, act) in &requested_moves {
            match *act {
                Action::KeepCrossingPath => {
                    if self.peds
                        .get_mut(&id)
                        .unwrap()
                        .step_cross_path(delta_time, map)
                    {
                        self.peds.remove(&id);
                    }
                }
                Action::StartCrossingPath => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.front_path = Some(CrossingFrontPath {
                        bldg: p.goal,
                        dist_along: map.get_b(p.goal).front_path.line.length(),
                        going_to_sidewalk: false,
                    });
                }
                Action::Continue => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.step_continue(delta_time, map);
                }
                Action::Goto(on) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    p.step_goto(on, map, intersections)?;
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

        Ok(())
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
        id: PedestrianID,
        start_bldg: BuildingID,
        goal_bldg: BuildingID,
        map: &Map,
        mut path: VecDeque<LaneID>,
    ) {
        let start = path.pop_front().unwrap();
        let front_path = &map.get_b(start_bldg).front_path;
        assert_eq!(start, front_path.sidewalk);
        let contraflow = is_contraflow(map, start, path[0]);
        self.peds.insert(
            id,
            Pedestrian {
                id,
                path,
                contraflow,
                on: On::Lane(start),
                dist_along: front_path.dist_along_sidewalk,
                waiting_for: None,
                front_path: Some(CrossingFrontPath {
                    bldg: start_bldg,
                    dist_along: 0.0 * si::M,
                    going_to_sidewalk: true,
                }),
                goal: goal_bldg,
            },
        );
        self.peds_per_sidewalk.insert(start, id);
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
}

fn is_contraflow(map: &Map, from: LaneID, to: LaneID) -> bool {
    map.get_l(from).dst_i != map.get_l(to).src_i
}
