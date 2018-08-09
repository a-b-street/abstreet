// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// This implements a simple driving model. Might adapt it into something nicer later, but for now,
// it exists to kind of enforce that driving models can be subbed out easily.

use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap};
use dimensioned::si;
use draw_car::DrawCar;
use geom::{Angle, Pt2D};
use intersections::{AgentInfo, IntersectionSimState, Request};
use kinematics::Vehicle;
use map_model::{LaneID, LaneType, Map, TurnID};
use models::{choose_turn, Action, FOLLOWING_DISTANCE};
use multimap::MultiMap;
use std::collections::{BTreeMap, HashSet, VecDeque};
use {AgentID, CarID, CarState, Distance, InvariantViolated, On, Tick};

// This represents an actively driving car, not a parked one
#[derive(Clone, Serialize, Deserialize)]
struct Car {
    id: CarID,
    on: On,
    // When did the car start the current On?
    started_at: Tick,
    waiting_for: Option<On>,
    debug: bool,
    // Head is the next lane
    path: VecDeque<LaneID>,
}

// TODO this is used for verifying sim state determinism, so it should actually check everything.
// the f64 prevents this from being derived.
impl PartialEq for Car {
    fn eq(&self, other: &Car) -> bool {
        self.id == other.id
    }
}
impl Eq for Car {}

impl Car {
    // Note this doesn't change the car's state, and it observes a fixed view of the world!
    fn react(
        &self,
        map: &Map,
        time: Tick,
        sim: &DrivingSimState,
        intersections: &IntersectionSimState,
    ) -> Action {
        let desired_on: On = {
            if let Some(on) = self.waiting_for {
                on
            } else {
                let dist = self.on.speed_limit(map) * (time - self.started_at).as_time();
                if dist < self.on.length(map) {
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
        // In a more detailed driving model, this would do things like lookahead.
        let has_room_now = match desired_on {
            On::Lane(id) => sim.lanes[id.0].room_at_end(time, &sim.cars, map),
            On::Turn(id) => sim.turns[&id].room_at_end(time, &sim.cars, map),
        };
        let is_lead_vehicle = match self.on {
            On::Lane(id) => sim.lanes[id.0].cars_queue[0] == self.id,
            On::Turn(id) => sim.turns[&id].cars_queue[0] == self.id,
        };
        let intersection_req_granted = match desired_on {
            // Already doing a turn, finish it!
            On::Lane(_) => true,
            On::Turn(id) => intersections.request_granted(Request::for_car(self.id, id)),
        };
        if has_room_now && is_lead_vehicle && intersection_req_granted {
            Action::Goto(desired_on)
        } else {
            Action::WaitFor(desired_on)
        }
    }

    fn step_goto(
        &mut self,
        on: On,
        time: Tick,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<(), InvariantViolated> {
        if let On::Turn(t) = self.on {
            intersections.on_exit(Request::for_car(self.id, t));
            assert_eq!(self.path[0], map.get_t(t).dst);
            self.path.pop_front();
        }
        self.waiting_for = None;
        self.on = on;
        if let On::Turn(t) = self.on {
            intersections.on_enter(Request::for_car(self.id, t))?;
        }
        // TODO could calculate leftover (and deal with large timesteps, small
        // lanes)
        self.started_at = time;
        Ok(())
    }

    // Returns the angle and the dist along the lane/turn too
    fn get_best_case_pos(&self, time: Tick, map: &Map) -> (Pt2D, Angle, Distance) {
        let mut dist = self.on.speed_limit(map) * (time - self.started_at).as_time();
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
    fn new(id: On, map: &Map) -> SimQueue {
        SimQueue {
            id,
            cars_queue: Vec::new(),
            capacity: ((id.length(map) / FOLLOWING_DISTANCE).ceil() as usize).max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    fn room_at_end(&self, time: Tick, cars: &BTreeMap<CarID, Car>, map: &Map) -> bool {
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
            >= FOLLOWING_DISTANCE / self.id.speed_limit(map)
    }

    fn reset(
        &mut self,
        ids: &Vec<CarID>,
        cars: &BTreeMap<CarID, Car>,
        map: &Map,
    ) -> Result<(), InvariantViolated> {
        let old_queue = self.cars_queue.clone();

        assert!(ids.len() <= self.capacity);
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        self.cars_queue.sort_by_key(|id| cars[id].started_at);

        // assert here we're not squished together too much
        let min_dt = FOLLOWING_DISTANCE / self.id.speed_limit(map);
        for slice in self.cars_queue.windows(2) {
            let (c1, c2) = (slice[0], slice[1]);
            let (t1, t2) = (
                cars[&c1].started_at.as_time(),
                cars[&c2].started_at.as_time(),
            );
            if t2 - t1 < min_dt {
                return Err(InvariantViolated(format!("uh oh! on {:?}, reset to {:?} broke. min dt is {}, but we have {} at {} and {} at {}. dt is just {}. prev queue was {:?}", self.id, self.cars_queue, min_dt, c1, t1, c2, t2, t2 - t1, old_queue)));
            }
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    // TODO this starts cars with their front aligned with the end of the lane, sticking their back
    // into the intersection. :(
    fn get_draw_cars(&self, time: Tick, sim: &DrivingSimState, map: &Map) -> Vec<DrawCar> {
        if self.cars_queue.is_empty() {
            return Vec::new();
        }

        // TODO base this on actual speed ;)
        let stopping_dist = Vehicle::typical_car().stopping_distance(self.id.speed_limit(map));

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
            stopping_dist,
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
                    stopping_dist,
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
                    stopping_dist,
                ));
            }
        }

        results
    }
}

// This manages only actively driving cars
#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub struct DrivingSimState {
    // Using BTreeMap instead of HashMap so iteration is deterministic.
    cars: BTreeMap<CarID, Car>,
    lanes: Vec<SimQueue>,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    turns: BTreeMap<TurnID, SimQueue>,
    debug: Option<CarID>,
}

impl DrivingSimState {
    pub fn new(map: &Map) -> DrivingSimState {
        let mut s = DrivingSimState {
            cars: BTreeMap::new(),
            // TODO only driving ones
            lanes: map.all_lanes()
                .iter()
                .map(|l| SimQueue::new(On::Lane(l.id), map))
                .collect(),
            turns: BTreeMap::new(),
            debug: None,
        };
        for t in map.all_turns().values() {
            if !t.between_sidewalks {
                s.turns.insert(t.id, SimQueue::new(On::Turn(t.id), map));
            }
        }
        s
    }

    pub fn populate_info_for_intersections(&self, info: &mut AgentInfo, map: &Map) {
        for c in self.cars.values() {
            let id = AgentID::Car(c.id);
            info.speeds.insert(
                id,
                if c.waiting_for.is_some() {
                    0.0 * si::MPS
                } else {
                    c.on.speed_limit(map)
                },
            );
            info.leaders.insert(id);
        }
    }

    pub fn get_car_state(&self, c: CarID) -> CarState {
        if let Some(driving) = self.cars.get(&c) {
            if driving.waiting_for.is_none() {
                CarState::Moving
            } else {
                CarState::Stuck
            }
        } else {
            // Assume the caller isn't asking about a nonexistent car
            CarState::Parked
        }
    }

    pub fn get_active_and_waiting_count(&self) -> (usize, usize) {
        let waiting = self.cars
            .values()
            .filter(|c| c.waiting_for.is_some())
            .count();
        (waiting, self.cars.len())
    }

    pub fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>> {
        if let Some(c) = self.cars.get(&id) {
            Some(vec![
                format!("Car {:?}", id),
                format!("On {:?}, started at {:?}", c.on, c.started_at),
                format!("Committed to waiting for {:?}", c.waiting_for),
                format!("{} lanes left in path", c.path.len()),
            ])
        } else {
            None
        }
    }

    pub fn toggle_debug(&mut self, id: CarID) {
        if let Some(c) = self.debug {
            if c != id {
                self.cars.get_mut(&c).unwrap().debug = false;
            }
        }

        if let Some(car) = self.cars.get_mut(&id) {
            println!("{}", abstutil::to_json(car));
            car.debug = !car.debug;
            self.debug = Some(id);
        } else {
            println!("{} is parked somewhere", id);
        }
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

    pub fn step(
        &mut self,
        time: Tick,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<(), InvariantViolated> {
        // Could be concurrent, since this is deterministic.
        let mut requested_moves: Vec<(CarID, Action)> = Vec::new();
        for c in self.cars.values() {
            requested_moves.push((c.id, c.react(map, time, &self, intersections)));
        }

        // In AORTA, there was a split here -- react vs step phase. We're still following the same
        // thing, but it might be slightly more clear to express it differently?

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
                    // Order matters due to new_car_entered_this_step.
                    // Why is this needed?
                    // - could two cars enter the same lane from the same turn? proper lookahead
                    // behavior WILL fix this
                    // - could two cars enter the same lane from different turns? no, then
                    // conflicting turns are happening simultaneously!
                    // - could two cars enter the same turn? proper lookahead
                    // behavior and not submitting a request until being the leader vehice should
                    // fix
                    if new_car_entered_this_step.contains(&on) {
                        // The car thought they could go, but have to abort last-minute. We may
                        // need to set waiting_for, since the car didn't necessarily return WaitFor
                        // previously.
                        self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
                    } else {
                        new_car_entered_this_step.insert(on);
                        let c = self.cars.get_mut(&id).unwrap();
                        c.step_goto(on, time, map, intersections)?;
                    }
                }
                Action::WaitFor(on) => {
                    self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
                    if let On::Turn(t) = on {
                        // Note this is idempotent and does NOT grant the request.
                        intersections.submit_request(Request::for_car(*id, t))?;
                    }
                }
            }
        }

        // TODO could simplify this by only adjusting the SimQueues we need above

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
                l.reset(v, &self.cars, map)?;
            } else {
                l.reset(&Vec::new(), &self.cars, map)?;
            }
            //l.reset(cars_per_lane.get_vec(&l.id).unwrap_or_else(|| &Vec::new()), &self.cars);
        }
        for t in self.turns.values_mut() {
            if let Some(v) = cars_per_turn.get_vec(&t.id.as_turn()) {
                t.reset(v, &self.cars, map)?;
            } else {
                t.reset(&Vec::new(), &self.cars, map)?;
            }
        }

        Ok(())
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
        map: &Map,
    ) -> bool {
        let start = path.pop_front().unwrap();

        if !self.lanes[start.0].room_at_end(time, &self.cars, map) {
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

    pub fn get_draw_car(&self, id: CarID, time: Tick, map: &Map) -> Option<DrawCar> {
        let all = match self.cars.get(&id)?.on {
            On::Lane(l) => self.get_draw_cars_on_lane(l, time, map),
            On::Turn(t) => self.get_draw_cars_on_turn(t, time, map),
        };
        all.into_iter().find(|c| c.id == id)
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
