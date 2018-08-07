use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap};
use dimensioned::si;
use draw_car::DrawCar;
use intersections::{IntersectionSimState, Request};
use kinematics;
use kinematics::Vehicle;
use map_model::{LaneID, LaneType, Map, TurnID};
use models::{choose_turn, FOLLOWING_DISTANCE};
use multimap::MultiMap;
use ordered_float::NotNaN;
use std::collections::{BTreeMap, HashMap, VecDeque};
use {
    Acceleration, AgentID, CarID, CarState, Distance, InvariantViolated, On, Speed, Tick,
    SPEED_LIMIT,
};

// This represents an actively driving car, not a parked one
#[derive(Clone, Serialize, Deserialize)]
struct Car {
    id: CarID,
    on: On,
    speed: Speed,
    dist_along: Distance,
    // TODO need to fill this out now
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

enum Action {
    Vanish, // TODO start parking instead
    Continue(Acceleration, Vec<Request>),
}

impl Car {
    // Note this doesn't change the car's state, and it observes a fixed view of the world!
    fn react(
        &self,
        map: &Map,
        _time: Tick,
        sim: &DrivingSimState,
        intersections: &IntersectionSimState,
    ) -> Action {
        if self.path.is_empty() {
            return Action::Vanish;
        }

        let vehicle = Vehicle::typical_car();

        // TODO could wrap this state up
        let mut dist_to_lookahead = vehicle.max_lookahead_dist(self.speed, SPEED_LIMIT);
        let mut constraints: Vec<Acceleration> = Vec::new();
        let mut requests: Vec<Request> = Vec::new();
        let mut current_on = self.on;
        let mut current_dist_along = self.dist_along;
        let mut current_path = self.path.clone();
        let mut dist_scanned_ahead = 0.0 * si::M;

        loop {
            // Don't exceed the speed limit
            // TODO speed limit per road
            constraints.push(vehicle.accel_to_achieve_speed_in_one_tick(self.speed, SPEED_LIMIT));

            // Don't hit the vehicle in front of us
            if let Some(other) = sim.next_car_in_front_of(current_on, current_dist_along) {
                assert!(self != other);
                assert!(current_dist_along < other.dist_along);
                let dist_behind_other =
                    dist_scanned_ahead + (other.dist_along - current_dist_along);
                let accel = vehicle.accel_to_follow(
                    self.speed,
                    SPEED_LIMIT,
                    &vehicle,
                    dist_behind_other,
                    other.speed,
                );

                if self.id == CarID(1549) && other.id == CarID(59) {
                    println!(
                        "currently {} behind, while lead on {:?} and we're on {:?}. our speed {}. need to accel {} to be ok",
                        dist_behind_other, current_on, self.on, self.speed, accel
                    );
                }
                constraints.push(accel);

                /*constraints.push(vehicle.accel_to_follow(
                    self.speed,
                    SPEED_LIMIT,
                    &vehicle,
                    dist_behind_other,
                    other.speed,
                ));*/
            }

            // Stop for intersections?
            if let On::Lane(id) = current_on {
                let stop_at_end = if current_path.is_empty() {
                    true
                } else {
                    let req = Request::for_car(
                        self.id,
                        choose_turn(&current_path, &self.waiting_for, id, map),
                    );
                    requests.push(req.clone());
                    !intersections.request_granted(req)
                };
                if stop_at_end {
                    constraints.push(vehicle.accel_to_stop_in_dist(
                        self.speed,
                        current_on.length(map) - current_dist_along,
                    ));
                    // No use in further lookahead.
                    break;
                }
            }

            // Advance to the next step.
            let dist_this_step = current_on.length(map) - current_dist_along;
            dist_to_lookahead -= dist_this_step;
            if dist_to_lookahead <= 0.0 * si::M {
                break;
            }
            current_on = match current_on {
                On::Turn(t) => {
                    current_path.pop_front();
                    On::Lane(map.get_t(t).dst)
                }
                On::Lane(l) => On::Turn(choose_turn(&current_path, &self.waiting_for, l, map)),
            };
            current_dist_along = 0.0 * si::M;
            dist_scanned_ahead += dist_this_step;
        }

        // TODO this type mangling is awful
        let safe_accel = constraints
            .into_iter()
            .min_by_key(|a| NotNaN::new(a.value_unsafe).unwrap())
            .unwrap();
        Action::Continue(safe_accel, requests)
    }

    fn step_continue(
        &mut self,
        accel: Acceleration,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) -> Result<(), InvariantViolated> {
        let (dist, new_speed) = kinematics::results_of_accel_for_one_tick(self.speed, accel);
        self.dist_along += dist;
        self.speed = new_speed;

        loop {
            let leftover_dist = self.dist_along - self.on.length(map);
            // == 0.0 is important! If no floating point imprecision happens, cars will stop RIGHT
            // at the end of a lane, with exactly 0 leftover distance. We don't want to bump them
            // into the turn and illegally enter the intersection in that case. The alternative
            // from AORTA, IIRC, is to make cars stop anywhere in a small buffer at the end of the
            // lane.
            if leftover_dist <= 0.0 * si::M {
                break;
            }
            let next_on = match self.on {
                On::Turn(t) => On::Lane(map.get_t(t).dst),
                On::Lane(l) => On::Turn(choose_turn(&self.path, &self.waiting_for, l, map)),
            };

            if let On::Turn(t) = self.on {
                intersections.on_exit(Request::for_car(self.id, t));
                assert_eq!(self.path[0], map.get_t(t).dst);
                self.path.pop_front();
            }
            self.waiting_for = None;
            self.on = next_on;
            if let On::Turn(t) = self.on {
                // TODO easier way to attach more debug info?
                intersections
                    .on_enter(Request::for_car(self.id, t))
                    .map_err(|e| {
                        InvariantViolated(format!(
                            "{}. new speed {}, leftover dist {}",
                            e, self.speed, leftover_dist
                        ))
                    })?;
            }
            self.dist_along = leftover_dist;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct SimQueue {
    id: On,
    // First element is farthest along the queue; they have the greatest dist_along.
    cars_queue: Vec<CarID>,
    capacity: usize,
}

impl SimQueue {
    fn new(id: On, map: &Map) -> SimQueue {
        SimQueue {
            id,
            cars_queue: Vec::new(),
            capacity: ((id.length(map) / FOLLOWING_DISTANCE).floor() as usize).max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    fn reset(
        &mut self,
        ids: &Vec<CarID>,
        cars: &BTreeMap<CarID, Car>,
    ) -> Result<(), InvariantViolated> {
        let old_queue = self.cars_queue.clone();

        assert_le!(ids.len(), self.capacity);
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        // Sort descending.
        self.cars_queue
            .sort_by_key(|id| -NotNaN::new(cars[id].dist_along.value_unsafe).unwrap());

        // assert here we're not squished together too much
        for slice in self.cars_queue.windows(2) {
            let (c1, c2) = (slice[0], slice[1]);
            let (dist1, dist2) = (cars[&c1].dist_along, cars[&c2].dist_along);
            if dist1 - dist2 < FOLLOWING_DISTANCE {
                return Err(InvariantViolated(format!("uh oh! on {:?}, reset to {:?} broke. min following distance is {}, but we have {} at {} and {} at {}. dist btwn is just {}. prev queue was {:?}", self.id, self.cars_queue, FOLLOWING_DISTANCE, c1, dist1, c2, dist2, dist1 - dist2, old_queue)));
            }
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    // TODO this starts cars with their front aligned with the end of the lane, sticking their back
    // into the intersection. :(
    fn get_draw_cars(&self, sim: &DrivingSimState, map: &Map) -> Vec<DrawCar> {
        let mut results = Vec::new();
        for id in &self.cars_queue {
            results.push(sim.get_draw_car(*id, Tick::zero(), map).unwrap())
        }
        results
    }

    fn next_car_in_front_of(&self, dist: Distance, sim: &DrivingSimState) -> Option<CarID> {
        self.cars_queue
            .iter()
            .rev()
            .find(|id| sim.cars[id].dist_along > dist)
            .map(|id| *id)
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

    pub fn get_all_speeds(&self) -> HashMap<AgentID, Speed> {
        let mut m = HashMap::new();
        for c in self.cars.values() {
            m.insert(AgentID::Car(c.id), c.speed);
        }
        m
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
                format!(
                    "On {:?}, speed {:?}, dist along {:?}",
                    c.on, c.speed, c.dist_along
                ),
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
            requested_moves.push((c.id, c.react(map, time, self, intersections)));
        }

        // In AORTA, there was a split here -- react vs step phase. We're still following the same
        // thing, but it might be slightly more clear to express it differently?

        // Apply moves. This should resolve in no conflicts because lookahead behavior works, so
        // this could be applied concurrently!
        for (id, act) in &requested_moves {
            match *act {
                Action::Vanish => {
                    self.cars.remove(&id);
                }
                Action::Continue(accel, ref requests) => {
                    let c = self.cars.get_mut(&id).unwrap();
                    c.step_continue(accel, map, intersections)?;
                    // TODO maybe just return TurnID
                    for req in requests {
                        // Note this is idempotent and does NOT grant the request.
                        // TODO should we check that the car is currently the lead vehicle?
                        // intersection is assuming that! or relax that assumption.
                        intersections.submit_request(req.clone())?;
                        //self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
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
                l.reset(v, &self.cars)?;
            } else {
                l.reset(&Vec::new(), &self.cars)?;
            }
            //l.reset(cars_per_lane.get_vec(&l.id).unwrap_or_else(|| &Vec::new()), &self.cars);
        }
        for t in self.turns.values_mut() {
            if let Some(v) = cars_per_turn.get_vec(&t.id.as_turn()) {
                t.reset(v, &self.cars)?;
            } else {
                t.reset(&Vec::new(), &self.cars)?;
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
        _time: Tick,
        car: CarID,
        mut path: VecDeque<LaneID>,
    ) -> bool {
        let start = path.pop_front().unwrap();

        self.cars.insert(
            car,
            Car {
                id: car,
                path,
                dist_along: 0.0 * si::M,
                speed: 0.0 * si::MPS,
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

    pub fn get_draw_car(&self, id: CarID, _time: Tick, map: &Map) -> Option<DrawCar> {
        let c = self.cars.get(&id)?;
        let (pos, angle) = c.on.dist_along(c.dist_along, map);
        let stopping_dist = Vehicle::typical_car().stopping_distance(c.speed);
        Some(DrawCar::new(
            c.id,
            c.waiting_for.and_then(|on| on.maybe_turn()),
            map,
            pos,
            angle,
            stopping_dist,
        ))
    }

    pub fn get_draw_cars_on_lane(&self, lane: LaneID, _time: Tick, map: &Map) -> Vec<DrawCar> {
        self.lanes[lane.0].get_draw_cars(self, map)
    }

    pub fn get_draw_cars_on_turn(&self, turn: TurnID, _time: Tick, map: &Map) -> Vec<DrawCar> {
        if let Some(queue) = self.turns.get(&turn) {
            return queue.get_draw_cars(self, map);
        }
        return Vec::new();
    }

    fn next_car_in_front_of(&self, on: On, dist: Distance) -> Option<&Car> {
        match on {
            On::Lane(id) => self.lanes[id.0].next_car_in_front_of(dist, self),
            On::Turn(id) => self.turns[&id].next_car_in_front_of(dist, self),
        }.map(|id| &self.cars[&id])
    }
}
