// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use draw_car::DrawCar;
use geom::{Angle, Pt2D};
use map_model::{Map, RoadID, TurnID};
use std;
use std::collections::{BTreeMap, VecDeque};
use std::f64;
use straw_model::Sim;
use {CarID, Tick, SPEED_LIMIT};

const FOLLOWING_DISTANCE: si::Meter<f64> = si::Meter {
    value_unsafe: 8.0,
    _marker: std::marker::PhantomData,
};

// TODO this name isn't quite right :)
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub(crate) enum On {
    Road(RoadID),
    Turn(TurnID),
}

impl On {
    pub(crate) fn as_road(&self) -> RoadID {
        match self {
            &On::Road(id) => id,
            &On::Turn(_) => panic!("not a road"),
        }
    }

    pub(crate) fn as_turn(&self) -> TurnID {
        match self {
            &On::Turn(id) => id,
            &On::Road(_) => panic!("not a turn"),
        }
    }

    fn length(&self, map: &Map) -> si::Meter<f64> {
        match self {
            &On::Road(id) => map.get_r(id).length(),
            &On::Turn(id) => map.get_t(id).length(),
        }
    }

    fn dist_along(&self, dist: si::Meter<f64>, map: &Map) -> (Pt2D, Angle) {
        match self {
            &On::Road(id) => map.get_r(id).dist_along(dist),
            &On::Turn(id) => map.get_t(id).dist_along(dist),
        }
    }
}

// This represents an actively driving car, not a parked one
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Car {
    // TODO might be going back to something old here, but an enum with parts of the state grouped
    // could be more clear.
    pub(crate) id: CarID,
    pub(crate) on: On,
    // When did the car start the current On?
    pub(crate) started_at: Tick,
    // TODO ideally, something else would remember Goto was requested and not even call step()
    pub(crate) waiting_for: Option<On>,
    pub(crate) debug: bool,
    // Head is the next road
    pub(crate) path: VecDeque<RoadID>,
}

pub(crate) enum Action {
    Vanish,      // hit a deadend, oops
    Continue,    // need more time to cross the current spot
    Goto(On),    // go somewhere if there's room
    WaitFor(On), // TODO this is only used inside sim. bleh.
}

impl Car {
    pub(crate) fn tooltip_lines(&self) -> Vec<String> {
        vec![
            format!("Car {:?}", self.id),
            format!("On {:?}, started at {:?}", self.on, self.started_at),
            format!("Committed to waiting for {:?}", self.waiting_for),
            format!("{} roads left in path", self.path.len()),
        ]
    }

    pub(crate) fn step(&self, map: &Map, time: Tick) -> Action {
        if let Some(on) = self.waiting_for {
            return Action::Goto(on);
        }

        let dist = SPEED_LIMIT * (time - self.started_at).as_time();
        if dist < self.on.length(map) {
            return Action::Continue;
        }

        // Done!
        if self.path.is_empty() {
            return Action::Vanish;
        }

        match self.on {
            // TODO cant try to go to next road unless we're the front car
            // if we dont do this here, we wont be able to see what turns people are waiting for
            // even if we wait till we're the front car, we might unravel the line of queued cars
            // too quickly
            On::Road(id) => Action::Goto(On::Turn(self.choose_turn(id, map))),
            On::Turn(id) => Action::Goto(On::Road(map.get_t(id).dst)),
        }
    }

    fn choose_turn(&self, from: RoadID, map: &Map) -> TurnID {
        assert!(self.waiting_for.is_none());
        for t in map.get_turns_from_road(from) {
            if t.dst == self.path[0] {
                return t.id;
            }
        }
        panic!("No turn from {} to {}", from, self.path[0]);
    }

    // Returns the angle and the dist along the road/turn too
    fn get_best_case_pos(&self, time: Tick, map: &Map) -> (Pt2D, Angle, si::Meter<f64>) {
        let mut dist = SPEED_LIMIT * (time - self.started_at).as_time();
        if self.waiting_for.is_some() {
            dist = self.on.length(map);
        }
        let (pt, angle) = self.on.dist_along(dist, map);
        (pt, angle, dist)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SimQueue {
    pub(crate) id: On,
    pub(crate) cars_queue: Vec<CarID>,
    capacity: usize,
}

impl SimQueue {
    pub(crate) fn new(id: On, map: &Map) -> SimQueue {
        SimQueue {
            id,
            cars_queue: Vec::new(),
            capacity: ((id.length(map) / FOLLOWING_DISTANCE).floor() as usize).max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    pub(crate) fn room_at_end(&self, time: Tick, cars: &BTreeMap<CarID, Car>) -> bool {
        if self.cars_queue.is_empty() {
            return true;
        }
        if self.cars_queue.len() == self.capacity {
            return false;
        }
        // Has the last car crossed at least FOLLOWING_DISTANCE? If so and the capacity
        // isn't filled, then we know for sure that there's room, because in this model, we assume
        // none of the cars just arbitrarily slow down or stop without reason.
        (time - cars[self.cars_queue.last().unwrap()].started_at).as_time()
            >= FOLLOWING_DISTANCE / SPEED_LIMIT
    }

    pub(crate) fn reset(&mut self, ids: &Vec<CarID>, cars: &BTreeMap<CarID, Car>) {
        let old_queue = self.cars_queue.clone();

        assert!(ids.len() <= self.capacity);
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        self.cars_queue.sort_by_key(|id| cars[id].started_at);

        // assert here we're not squished together too much
        let min_dt = FOLLOWING_DISTANCE / SPEED_LIMIT;
        for slice in self.cars_queue.windows(2) {
            let c1 = cars[&slice[0]].started_at.as_time();
            let c2 = cars[&slice[1]].started_at.as_time();
            if c2 - c1 < min_dt {
                println!("uh oh! on {:?}, reset to {:?} broke. min dt is {}, but we have {} and {}. badness {}", self.id, self.cars_queue, min_dt, c2, c1, c2 - c1 - min_dt);
                println!("  prev queue was {:?}", old_queue);
                for c in &self.cars_queue {
                    println!("  {:?} started at {}", c, cars[c].started_at);
                }
                panic!("invariant borked");
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    // TODO this starts cars with their front aligned with the end of the road, sticking their back
    // into the intersection. :(
    pub(crate) fn get_draw_cars(&self, sim: &Sim, map: &Map) -> Vec<DrawCar> {
        if self.cars_queue.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let (pos1, angle1, dist_along1) =
            sim.cars[&self.cars_queue[0]].get_best_case_pos(sim.time, map);
        results.push(DrawCar::new(
            &sim.cars[&self.cars_queue[0]],
            map,
            pos1,
            angle1,
        ));
        let mut dist_along_bound = dist_along1;

        for id in self.cars_queue.iter().skip(1) {
            let (pos, angle, dist_along) = sim.cars[id].get_best_case_pos(sim.time, map);
            if dist_along_bound - FOLLOWING_DISTANCE > dist_along {
                results.push(DrawCar::new(&sim.cars[id], map, pos, angle));
                dist_along_bound = dist_along;
            } else {
                dist_along_bound -= FOLLOWING_DISTANCE;
                // If not, we violated room_at_end() and reset() didn't catch it
                assert!(dist_along_bound >= 0.0 * si::M, "dist_along_bound went negative ({}) for {:?} (length {}) with queue {:?}. first car at {}", dist_along_bound, self.id, self.id.length(map), self.cars_queue, dist_along1);
                let (pt, angle) = self.id.dist_along(dist_along_bound, map);
                results.push(DrawCar::new(&sim.cars[id], map, pt, angle));
            }
        }

        results
    }
}
