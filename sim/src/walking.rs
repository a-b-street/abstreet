use abstutil;
use abstutil::{deserialize_multimap, serialize_multimap};
use dimensioned::si;
use draw_ped::DrawPedestrian;
use intersections::{IntersectionSimState, Request};
use map_model::{Lane, LaneID, Map, Turn, TurnID};
use models::{choose_turn, Action};
use multimap::MultiMap;
use std;
use std::collections::{BTreeMap, VecDeque};
use {On, PedestrianID, Tick};

// TODO tune these!
// TODO make it vary, after we can easily serialize these
// TODO temporarily very high to debug peds faster
const SPEED: si::MeterPerSecond<f64> = si::MeterPerSecond {
    value_unsafe: 3.9,
    _marker: std::marker::PhantomData,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pedestrian {
    id: PedestrianID,

    on: On,
    // TODO since Tick is deliberately not f64, have a better type for Meters.
    dist_along: si::Meter<f64>,
    // Traveling along the lane/turn in its original direction or not?
    contraflow: bool,

    // Head is the next lane
    path: VecDeque<LaneID>,
    waiting_for: Option<On>,
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
        let desired_on: On = {
            if let Some(on) = self.waiting_for {
                on
            } else {
                if (!self.contraflow && self.dist_along < self.on.length(map))
                    || (self.contraflow && self.dist_along > 0.0 * si::M)
                {
                    return Action::Continue;
                }

                // Done!
                if self.path.is_empty() {
                    return Action::Vanish;
                }

                match self.on {
                    On::Lane(id) => On::Turn(choose_turn(&self.path, &self.waiting_for, id, map)),
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
}

#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    peds_per_sidewalk: MultiMap<LaneID, PedestrianID>,
    #[serde(serialize_with = "serialize_multimap")]
    #[serde(deserialize_with = "deserialize_multimap")]
    peds_per_turn: MultiMap<TurnID, PedestrianID>,

    id_counter: usize,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_sidewalk: MultiMap::new(),
            peds_per_turn: MultiMap::new(),
            id_counter: 0,
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

    pub fn total_count(&self) -> usize {
        self.id_counter
    }

    pub fn step(
        &mut self,
        time: Tick,
        delta_time: si::Second<f64>,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) {
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
                Action::Vanish => {
                    self.peds.remove(&id);
                }
                Action::Continue => {
                    let p = self.peds.get_mut(&id).unwrap();
                    let new_dist: si::Meter<f64> = delta_time * SPEED;
                    if p.contraflow {
                        p.dist_along -= new_dist;
                        if p.dist_along < 0.0 * si::M {
                            p.dist_along = 0.0 * si::M;
                        }
                    } else {
                        p.dist_along += new_dist;
                        let max_dist = p.on.length(map);
                        if p.dist_along > max_dist {
                            p.dist_along = max_dist;
                        }
                    }
                }
                Action::Goto(on) => {
                    let p = self.peds.get_mut(&id).unwrap();
                    let old_on = p.on.clone();
                    if let On::Turn(t) = p.on {
                        intersections.on_exit(Request::for_ped(p.id, t));
                        assert_eq!(p.path[0], map.get_t(t).dst);
                        p.path.pop_front();
                    }
                    p.waiting_for = None;
                    p.on = on;
                    p.dist_along = 0.0 * si::M;
                    p.contraflow = false;
                    match p.on {
                        On::Turn(t) => {
                            intersections.on_enter(Request::for_ped(p.id, t));
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
                                p.contraflow = true;
                                p.dist_along = lane.length();
                            }
                        }
                    }

                    // TODO could calculate leftover (and deal with large timesteps, small
                    // lanes)
                }
                Action::WaitFor(on) => {
                    self.peds.get_mut(&id).unwrap().waiting_for = Some(on);
                    if let On::Turn(t) = on {
                        // Note this is idempotent and does NOT grant the request.
                        intersections.submit_request(Request::for_ped(*id, t), time);
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
            ped.on.dist_along(ped.dist_along, map).0,
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
                l.dist_along(ped.dist_along).0,
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

    pub fn seed_pedestrian(&mut self, map: &Map, mut path: VecDeque<LaneID>) {
        let id = PedestrianID(self.id_counter);
        self.id_counter += 1;

        let start = path.pop_front().unwrap();
        let contraflow = is_contraflow(map, start, path[0]);
        self.peds.insert(
            id,
            Pedestrian {
                id,
                path,
                contraflow,
                on: On::Lane(start),
                // TODO start next to a building path, or at least some random position
                dist_along: 0.0 * si::M,
                waiting_for: None,
            },
        );
        self.peds_per_sidewalk.insert(start, id);
    }
}

fn is_contraflow(map: &Map, from: LaneID, to: LaneID) -> bool {
    map.get_l(from).dst_i != map.get_l(to).src_i
}
