use control::ControlMap;
use dimensioned::si;
use draw_ped::DrawPedestrian;
use map_model::{LaneType, Map, Road, RoadID, Turn, TurnID};
use multimap::MultiMap;
use rand::Rng;
use std;
use std::collections::VecDeque;
use {pick_goal_and_find_path, On, PedestrianID, Tick};

// TODO tune these!
// TODO make it vary, after we can easily serialize these
const SPEED: si::MeterPerSecond<f64> = si::MeterPerSecond {
    value_unsafe: 0.9,
    _marker: std::marker::PhantomData,
};

#[derive(Serialize, Deserialize)]
struct Pedestrian {
    id: PedestrianID,

    on: On,
    // TODO si::Meter<f64> after serde support lands
    // TODO or since Tick is deliberately not f64, have a better type for Meters.
    dist_along: f64,
    // Traveling along the road/turn in its original direction or not?
    contraflow: bool,

    // Head is the next road
    path: VecDeque<RoadID>,
}

// TODO this is used for verifying sim state determinism, so it should actually check everything.
// the f64 prevents this from being derived.
impl PartialEq for Pedestrian {
    fn eq(&self, other: &Pedestrian) -> bool {
        self.id == other.id
    }
}
impl Eq for Pedestrian {}

#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub(crate) struct WalkingSimState {
    // Trying a different style than driving for storing things
    peds_per_sidewalk: MultiMap<RoadID, Pedestrian>,
    peds_per_turn: MultiMap<TurnID, Pedestrian>,

    id_counter: usize,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds_per_sidewalk: MultiMap::new(),
            peds_per_turn: MultiMap::new(),
            id_counter: 0,
        }
    }

    pub fn step(&mut self, time: Tick, map: &Map, control_map: &ControlMap) {
        // TODO implement
    }

    pub fn get_draw_peds_on_road(&self, r: &Road) -> Vec<DrawPedestrian> {
        let mut result = Vec::new();
        for p in self.peds_per_sidewalk.get_vec(&r.id).unwrap_or(&Vec::new()) {
            result.push(DrawPedestrian::new(
                p.id,
                r.dist_along(p.dist_along * si::M).0,
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

    pub fn seed_pedestrians<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        map: &Map,
        num_peds: usize,
    ) -> usize {
        let mut sidewalks: Vec<RoadID> = Vec::new();
        for r in map.all_roads() {
            if r.lane_type == LaneType::Sidewalk {
                sidewalks.push(r.id);
            }
        }

        let mut count = 0;
        for _i in 0..num_peds {
            let start = *rng.choose(&sidewalks).unwrap();
            if self.seed_pedestrian(rng, map, start) {
                count += 1;
            }
        }
        count
    }

    pub fn seed_pedestrian<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        map: &Map,
        start: RoadID,
    ) -> bool {
        if let Some(path) = pick_goal_and_find_path(rng, map, start) {
            let id = PedestrianID(self.id_counter);
            self.id_counter += 1;
            let contraflow = is_contraflow(map, start, path[0]);
            self.peds_per_sidewalk.insert(
                start,
                Pedestrian {
                    id,
                    path,
                    contraflow,
                    on: On::Road(start),
                    // TODO start next to a building path, or at least some random position
                    dist_along: 0.0,
                },
            );
            true
        } else {
            false
        }
    }
}

fn is_contraflow(map: &Map, from: RoadID, to: RoadID) -> bool {
    map.get_r(from).dst_i != map.get_r(to).src_i
}
