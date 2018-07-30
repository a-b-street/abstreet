use control::ControlMap;
use dimensioned::si;
use draw_ped::DrawPedestrian;
use map_model::{Lane, LaneID, Map, Turn, TurnID};
use multimap::MultiMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std;
use std::collections::VecDeque;
use {On, PedestrianID};

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
    // TODO si::Meter<f64> after serde support lands
    // TODO or since Tick is deliberately not f64, have a better type for Meters.
    dist_along: f64,
    // Traveling along the lane/turn in its original direction or not?
    contraflow: bool,

    // Head is the next lane
    path: VecDeque<LaneID>,
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
    // True if done and should vanish!
    fn step_sidewalk(
        &mut self,
        delta_time: si::Second<f64>,
        map: &Map,
        _control_map: &ControlMap,
    ) -> bool {
        let new_dist: si::Meter<f64> = delta_time * SPEED;
        let done_current_sidewalk = if self.contraflow {
            self.dist_along -= new_dist.value_unsafe;
            self.dist_along <= 0.0
        } else {
            self.dist_along += new_dist.value_unsafe;
            self.dist_along * si::M >= self.on.length(map)
        };

        if !done_current_sidewalk {
            return false;
        }
        if self.path.is_empty() {
            return true;
        }

        let turn = map.get_turns_from_lane(self.on.as_lane())
            .iter()
            .find(|t| t.dst == self.path[0])
            .unwrap()
            .id;
        // TODO request the turn and wait for it; don't just go!
        self.on = On::Turn(turn);
        self.contraflow = false;
        self.dist_along = 0.0;
        self.path.pop_front();
        false
    }

    fn step_turn(&mut self, delta_time: si::Second<f64>, map: &Map, _control_map: &ControlMap) {
        let new_dist: si::Meter<f64> = delta_time * SPEED;
        self.dist_along += new_dist.value_unsafe;
        if self.dist_along * si::M < self.on.length(map) {
            return;
        }

        let turn = map.get_t(self.on.as_turn());
        let lane = map.get_l(turn.dst);
        self.on = On::Lane(lane.id);

        // Which end of the sidewalk are we entering?
        // TODO are there cases where we should enter a new sidewalk and immediately enter a
        // different turn, instead of always going to the other side of the sidealk? or are there
        // enough turns to make that unnecessary?
        if turn.parent == lane.src_i {
            self.contraflow = false;
            self.dist_along = 0.0;
        } else {
            self.contraflow = true;
            self.dist_along = lane.length().value_unsafe;
        }
    }
}

#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub struct WalkingSimState {
    // Trying a different style than driving for storing things
    peds_per_sidewalk: MultiMap<LaneID, Pedestrian>,
    #[serde(serialize_with = "serialize_multimap")]
    #[serde(deserialize_with = "deserialize_multimap")]
    peds_per_turn: MultiMap<TurnID, Pedestrian>,

    id_counter: usize,
}

// TODO make generic, lift to abstutil
fn serialize_multimap<S: Serializer>(
    map: &MultiMap<TurnID, Pedestrian>,
    s: S,
) -> Result<S::Ok, S::Error> {
    // TODO maybe need to sort by TurnID to have deterministic output
    map.iter_all()
        .map(|(key, values)| (key.clone(), values.clone()))
        .collect::<Vec<(_, _)>>()
        .serialize(s)
}
fn deserialize_multimap<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<MultiMap<TurnID, Pedestrian>, D::Error> {
    let vec = <Vec<(TurnID, Vec<Pedestrian>)>>::deserialize(d)?;
    let mut map = MultiMap::new();
    for (key, values) in vec {
        for value in values {
            map.insert(key, value);
        }
    }
    Ok(map)
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
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

    pub fn step(&mut self, delta_time: si::Second<f64>, map: &Map, control_map: &ControlMap) {
        // Since pedestrians don't interact at all, any ordering and concurrency is deterministic
        // here.
        // TODO but wait, the interactions with the intersections aren't deterministic!

        // TODO not sure how to do this most fluidly and performantly. might even make more sense
        // to just have a slotmap of peds, then a multimap from lane->ped IDs to speed up drawing.
        // since we seemingly can't iterate and consume a MultiMap, slotmap really seems best.
        let mut new_per_sidewalk: MultiMap<LaneID, Pedestrian> = MultiMap::new();
        let mut new_per_turn: MultiMap<TurnID, Pedestrian> = MultiMap::new();

        for (_, peds) in self.peds_per_sidewalk.iter_all_mut() {
            for p in peds.iter_mut() {
                if !p.step_sidewalk(delta_time, map, control_map) {
                    match p.on {
                        On::Lane(id) => new_per_sidewalk.insert(id, p.clone()),
                        On::Turn(id) => new_per_turn.insert(id, p.clone()),
                    };
                }
            }
        }
        for (_, peds) in self.peds_per_turn.iter_all_mut() {
            for p in peds.iter_mut() {
                p.step_turn(delta_time, map, control_map);
                match p.on {
                    On::Lane(id) => new_per_sidewalk.insert(id, p.clone()),
                    On::Turn(id) => new_per_turn.insert(id, p.clone()),
                };
            }
        }

        self.peds_per_sidewalk = new_per_sidewalk;
        self.peds_per_turn = new_per_turn;
    }

    pub fn get_draw_peds_on_lane(&self, l: &Lane) -> Vec<DrawPedestrian> {
        let mut result = Vec::new();
        for p in self.peds_per_sidewalk.get_vec(&l.id).unwrap_or(&Vec::new()) {
            result.push(DrawPedestrian::new(
                p.id,
                l.dist_along(p.dist_along * si::M).0,
            ));
        }
        result
    }

    pub fn get_draw_peds_on_turn(&self, t: &Turn) -> Vec<DrawPedestrian> {
        let mut result = Vec::new();
        for p in self.peds_per_turn.get_vec(&t.id).unwrap_or(&Vec::new()) {
            result.push(DrawPedestrian::new(
                p.id,
                t.dist_along(p.dist_along * si::M).0,
            ));
        }
        result
    }

    pub fn seed_pedestrian(&mut self, map: &Map, mut path: VecDeque<LaneID>) {
        let id = PedestrianID(self.id_counter);
        self.id_counter += 1;

        let start = path.pop_front().unwrap();
        let contraflow = is_contraflow(map, start, path[0]);
        self.peds_per_sidewalk.insert(
            start,
            Pedestrian {
                id,
                path,
                contraflow,
                on: On::Lane(start),
                // TODO start next to a building path, or at least some random position
                dist_along: 0.0,
            },
        );
    }
}

fn is_contraflow(map: &Map, from: LaneID, to: LaneID) -> bool {
    map.get_l(from).dst_i != map.get_l(to).src_i
}
