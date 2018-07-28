// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use control::ControlMap;
use dimensioned::si;
use draw_car::DrawCar;
use geom::{Angle, Pt2D};
use intersections::{IntersectionPolicy, StopSign, TrafficSignal};
use map_model::{LaneID, LaneType, Map, TurnID};
use multimap::MultiMap;
use std;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::f64;
use {CarID, On, Tick, SPEED_LIMIT};

const FOLLOWING_DISTANCE: si::Meter<f64> = si::Meter {
    value_unsafe: 8.0,
    _marker: std::marker::PhantomData,
};

// This represents an actively driving car, not a parked one
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Car {
    // TODO might be going back to something old here, but an enum with parts of the state grouped
    // could be more clear.
    pub id: CarID,
    pub on: On,
    // When did the car start the current On?
    pub started_at: Tick,
    // TODO ideally, something else would remember Goto was requested and not even call step()
    pub waiting_for: Option<On>,
    pub debug: bool,
    // Head is the next lane
    pub path: VecDeque<LaneID>,
}

enum Action {
    Vanish,      // hit a deadend, oops
    Continue,    // need more time to cross the current spot
    Goto(On),    // go somewhere if there's room
    WaitFor(On), // TODO this is only used inside sim. bleh.
}

impl Car {
    pub fn tooltip_lines(&self) -> Vec<String> {
        vec![
            format!("Car {:?}", self.id),
            format!("On {:?}, started at {:?}", self.on, self.started_at),
            format!("Committed to waiting for {:?}", self.waiting_for),
            format!("{} lanes left in path", self.path.len()),
        ]
    }

    fn step(&self, map: &Map, time: Tick) -> Action {
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
            // TODO cant try to go to next lane unless we're the front car
            // if we dont do this here, we wont be able to see what turns people are waiting for
            // even if we wait till we're the front car, we might unravel the line of queued cars
            // too quickly
            On::Lane(id) => Action::Goto(On::Turn(self.choose_turn(id, map))),
            On::Turn(id) => Action::Goto(On::Lane(map.get_t(id).dst)),
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

    // Returns the angle and the dist along the lane/turn too
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
struct SimQueue {
    id: On,
    cars_queue: Vec<CarID>,
    capacity: usize,
}

impl SimQueue {
    pub fn new(id: On, map: &Map) -> SimQueue {
        SimQueue {
            id,
            cars_queue: Vec::new(),
            capacity: ((id.length(map) / FOLLOWING_DISTANCE).floor() as usize).max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    pub fn room_at_end(&self, time: Tick, cars: &BTreeMap<CarID, Car>) -> bool {
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

    pub fn reset(&mut self, ids: &Vec<CarID>, cars: &BTreeMap<CarID, Car>) {
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

    pub fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    // TODO this starts cars with their front aligned with the end of the lane, sticking their back
    // into the intersection. :(
    pub fn get_draw_cars(&self, time: Tick, sim: &DrivingSimState, map: &Map) -> Vec<DrawCar> {
        if self.cars_queue.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let (pos1, angle1, dist_along1) =
            sim.cars[&self.cars_queue[0]].get_best_case_pos(time, map);
        results.push(DrawCar::new(
            self.cars_queue[0],
            sim.cars[&self.cars_queue[0]]
                .waiting_for
                .and_then(|on| on.maybe_turn()),
            map,
            pos1,
            angle1,
        ));
        let mut dist_along_bound = dist_along1;

        for id in self.cars_queue.iter().skip(1) {
            let (pos, angle, dist_along) = sim.cars[id].get_best_case_pos(time, map);
            if dist_along_bound - FOLLOWING_DISTANCE > dist_along {
                results.push(DrawCar::new(
                    *id,
                    sim.cars[id].waiting_for.and_then(|on| on.maybe_turn()),
                    map,
                    pos,
                    angle,
                ));
                dist_along_bound = dist_along;
            } else {
                dist_along_bound -= FOLLOWING_DISTANCE;
                // If not, we violated room_at_end() and reset() didn't catch it
                assert!(dist_along_bound >= 0.0 * si::M, "dist_along_bound went negative ({}) for {:?} (length {}) with queue {:?}. first car at {}", dist_along_bound, self.id, self.id.length(map), self.cars_queue, dist_along1);
                let (pt, angle) = self.id.dist_along(dist_along_bound, map);
                results.push(DrawCar::new(
                    *id,
                    sim.cars[id].waiting_for.and_then(|on| on.maybe_turn()),
                    map,
                    pt,
                    angle,
                ));
            }
        }

        results
    }
}

// This manages only actively driving cars
#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub struct DrivingSimState {
    // TODO investigate slot map-like structures for performance
    // Using BTreeMap instead of HashMap so iteration is deterministic. Should be able to relax
    // this later after step() doesnt need a RNG.
    pub(crate) cars: BTreeMap<CarID, Car>,
    lanes: Vec<SimQueue>,
    turns: BTreeMap<TurnID, SimQueue>,
    intersections: Vec<IntersectionPolicy>,
}

impl DrivingSimState {
    pub fn new(map: &Map) -> DrivingSimState {
        let mut intersections: Vec<IntersectionPolicy> = Vec::new();
        for i in map.all_intersections() {
            if i.has_traffic_signal {
                intersections.push(IntersectionPolicy::TrafficSignalPolicy(TrafficSignal::new(
                    i.id,
                )));
            } else {
                intersections.push(IntersectionPolicy::StopSignPolicy(StopSign::new(i.id)));
            }
        }

        let mut s = DrivingSimState {
            intersections,

            cars: BTreeMap::new(),
            // TODO only driving ones
            lanes: map.all_lanes()
                .iter()
                .map(|l| SimQueue::new(On::Lane(l.id), map))
                .collect(),
            turns: BTreeMap::new(),
        };
        for t in map.all_turns().values() {
            if !t.between_sidewalks {
                s.turns.insert(t.id, SimQueue::new(On::Turn(t.id), map));
            }
        }
        s
    }

    pub fn edit_remove_lane(&mut self, id: LaneID) {
        assert!(self.lanes[id.0].is_empty());
    }

    pub fn edit_add_lane(&mut self, id: LaneID) {
        assert!(self.lanes[id.0].is_empty());
    }

    pub fn edit_remove_turn(&mut self, id: TurnID) {
        if let Some(queue) = self.turns.get(&id) {
            assert!(queue.is_empty());
        }
        self.turns.remove(&id);
    }

    pub fn edit_add_turn(&mut self, id: TurnID, map: &Map) {
        self.turns.insert(id, SimQueue::new(On::Turn(id), map));
    }

    pub fn step(&mut self, time: Tick, map: &Map, control_map: &ControlMap) {
        // Could be concurrent, since this is deterministic. Note no RNG. Ask all cars for their
        // move, reinterpreting Goto to see if there's room now. It's important to query
        // has_room_now here using the previous, fixed state of the world. If we did it in the next
        // loop, then order of updates would matter for more than just conflict resolution.
        //
        // Note that since this uses RNG right now, it's only deterministic if iteration order is!
        // So can't be concurrent and use RNG. Could have a RNG per car or something later if we
        // really needed both.
        let mut requested_moves: Vec<(CarID, Action)> = Vec::new();
        for c in self.cars.values() {
            requested_moves.push((
                c.id,
                match c.step(map, time) {
                    Action::Goto(on) => {
                        // This is a monotonic property in conjunction with
                        // new_car_entered_this_step. The last car won't go backwards.
                        let has_room_now = match on {
                            On::Lane(id) => self.lanes[id.0].room_at_end(time, &self.cars),
                            On::Turn(id) => self.turns[&id].room_at_end(time, &self.cars),
                        };
                        let is_lead_vehicle = match c.on {
                            On::Lane(id) => self.lanes[id.0].cars_queue[0] == c.id,
                            On::Turn(id) => self.turns[&id].cars_queue[0] == c.id,
                        };
                        if has_room_now && is_lead_vehicle {
                            Action::Goto(on)
                        } else {
                            Action::WaitFor(on)
                        }
                    }
                    x => x,
                },
            ));
        }
        // TODO since self.cars is a hash, requested_moves is in random order. sort by car ID to be
        // deterministic.
        requested_moves.sort_by_key(|pair| (pair.0).0);

        // Apply moves, resolving conflicts. This has to happen serially.
        // It might make more sense to push the conflict resolution down to SimQueue?
        // TODO should shuffle deterministically here, to be more fair
        let mut new_car_entered_this_step = HashSet::new();
        for (id, act) in &requested_moves {
            match *act {
                Action::Vanish => {
                    self.cars.remove(&id);
                }
                Action::Continue => {}
                Action::Goto(on) => {
                    // Order matters due to can_do_turn being mutable and due to
                    // new_car_entered_this_step.
                    let mut ok_to_turn = true;
                    if let On::Turn(t) = on {
                        ok_to_turn = self.intersections[map.get_t(t).parent.0].can_do_turn(
                            *id,
                            t,
                            time,
                            map,
                            control_map,
                        );
                    }

                    if new_car_entered_this_step.contains(&on) || !ok_to_turn {
                        self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
                    } else {
                        new_car_entered_this_step.insert(on);
                        let c = self.cars.get_mut(&id).unwrap();
                        if let On::Turn(t) = c.on {
                            self.intersections[map.get_t(t).parent.0].on_exit(c.id);
                            assert_eq!(c.path[0], map.get_t(t).dst);
                            c.path.pop_front();
                        }
                        c.waiting_for = None;
                        c.on = on;
                        if let On::Turn(t) = c.on {
                            self.intersections[map.get_t(t).parent.0].on_enter(c.id);
                        }
                        // TODO could calculate leftover (and deal with large timesteps, small
                        // lanes)
                        c.started_at = time;
                    }
                }
                Action::WaitFor(on) => {
                    self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
                }
            }
        }

        // Group cars by lane and turn
        // TODO ideally, just hash On
        let mut cars_per_lane = MultiMap::new();
        let mut cars_per_turn = MultiMap::new();
        for c in self.cars.values() {
            match c.on {
                On::Lane(id) => cars_per_lane.insert(id, c.id),
                On::Turn(id) => cars_per_turn.insert(id, c.id),
            };
        }

        // Reset all queues
        for l in &mut self.lanes {
            if let Some(v) = cars_per_lane.get_vec(&l.id.as_lane()) {
                l.reset(v, &self.cars);
            } else {
                l.reset(&Vec::new(), &self.cars);
            }
            //l.reset(cars_per_lane.get_vec(&l.id).unwrap_or_else(|| &Vec::new()), &self.cars);
        }
        for t in self.turns.values_mut() {
            if let Some(v) = cars_per_turn.get_vec(&t.id.as_turn()) {
                t.reset(v, &self.cars);
            } else {
                t.reset(&Vec::new(), &self.cars);
            }
        }
    }

    // TODO cars basically start in the intersection, with their front bumper right at the
    // beginning of the lane. later, we want cars starting at arbitrary points in the middle of the
    // lane (from a building), so just ignore this problem for now.
    // True if we spawned one
    pub fn start_car_on_lane(
        &mut self,
        time: Tick,
        car: CarID,
        mut path: VecDeque<LaneID>,
    ) -> bool {
        let start = path.pop_front().unwrap();

        if !self.lanes[start.0].room_at_end(time, &self.cars) {
            // TODO car should enter Unparking state and wait for room
            println!("No room for {} to start driving on {}", car, start);
            return false;
        }

        self.cars.insert(
            car,
            Car {
                id: car,
                path,
                started_at: time,
                on: On::Lane(start),
                waiting_for: None,
                debug: false,
            },
        );
        self.lanes[start.0].cars_queue.push(car);
        true
    }

    pub fn get_empty_lanes(&self, map: &Map) -> Vec<LaneID> {
        let mut lanes: Vec<LaneID> = Vec::new();
        for (idx, queue) in self.lanes.iter().enumerate() {
            if map.get_l(LaneID(idx)).lane_type == LaneType::Driving && queue.is_empty() {
                lanes.push(queue.id.as_lane());
            }
        }
        lanes
    }

    pub fn get_draw_cars_on_lane(&self, lane: LaneID, time: Tick, map: &Map) -> Vec<DrawCar> {
        self.lanes[lane.0].get_draw_cars(time, self, map)
    }

    pub fn get_draw_cars_on_turn(&self, turn: TurnID, time: Tick, map: &Map) -> Vec<DrawCar> {
        if let Some(queue) = self.turns.get(&turn) {
            return queue.get_draw_cars(time, self, map);
        }
        return Vec::new();
    }
}
