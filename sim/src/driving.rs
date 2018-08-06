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
use std::collections::{BTreeMap, VecDeque};
use {Acceleration, CarID, CarState, Distance, On, Speed, Tick, SPEED_LIMIT};

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
    Continue(Acceleration, Option<Request>),
}

impl Car {
    // Note this doesn't change the car's state, and it observes a fixed view of the world!
    fn react(&self, map: &Map, _time: Tick, intersections: &IntersectionSimState) -> Action {
        if self.path.is_empty() {
            return Action::Vanish;
        }

        // TODO for all of these, do lookahead. max lookahead dist is bound by current road's speed
        // limit and... er, is it just the stopping distance? or the stopping distance assuming we
        // accelerate the max here?

        // Don't exceed the speed limit
        let constraint1 = Some(
            Vehicle::typical_car().accel_to_achieve_speed_in_one_tick(self.speed, SPEED_LIMIT),
        );

        // Stop for intersections if we have to
        let maybe_request = match self.on {
            On::Turn(_) => None,
            On::Lane(id) => Some(Request::for_car(
                self.id,
                choose_turn(&self.path, &self.waiting_for, id, map),
            )),
        };
        let constraint2 = if let Some(ref req) = maybe_request {
            if intersections.request_granted(req.clone()) {
                None
            } else {
                Some(
                    Vehicle::typical_car()
                        .accel_to_stop_in_dist(self.speed, self.on.length(map) - self.dist_along),
                )
            }
        } else {
            None
        };

        // TODO don't hit the vehicle in front of us

        // TODO this type mangling is awful
        let safe_accel = vec![constraint1, constraint2]
            .into_iter()
            .filter_map(|c| c)
            .min_by_key(|a| NotNaN::new(a.value_unsafe).unwrap())
            .unwrap();
        Action::Continue(safe_accel, maybe_request)
    }

    fn step_continue(
        &mut self,
        accel: Acceleration,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) {
        let (dist, new_speed) = kinematics::results_of_accel_for_one_tick(self.speed, accel);
        self.dist_along += dist;
        self.speed = new_speed;

        loop {
            let leftover_dist = self.dist_along - self.on.length(map);
            if leftover_dist < 0.0 * si::M {
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
                intersections.on_enter(Request::for_car(self.id, t));
            }
            self.dist_along = leftover_dist;
        }
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
            capacity: ((id.length(map) / FOLLOWING_DISTANCE).floor() as usize).max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    fn reset(&mut self, ids: &Vec<CarID>, cars: &BTreeMap<CarID, Car>) {
        let old_queue = self.cars_queue.clone();

        assert!(ids.len() <= self.capacity);
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        self.cars_queue
            .sort_by_key(|id| NotNaN::new(cars[id].dist_along.value_unsafe).unwrap());

        // assert here we're not squished together too much
        for slice in self.cars_queue.windows(2) {
            let c1 = cars[&slice[0]].dist_along;
            let c2 = cars[&slice[1]].dist_along;
            if c2 - c1 < FOLLOWING_DISTANCE {
                println!("uh oh! on {:?}, reset to {:?} broke. min following distance is {}, but we have {} and {}. badness {}", self.id, self.cars_queue, FOLLOWING_DISTANCE, c2, c1, c2 - c1 - FOLLOWING_DISTANCE);
                println!("  prev queue was {:?}", old_queue);
                panic!("invariant borked");
            }
        }
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

    pub fn step(&mut self, time: Tick, map: &Map, intersections: &mut IntersectionSimState) {
        // TODO choose acceleration, update speed

        // Could be concurrent, since this is deterministic.
        let mut requested_moves: Vec<(CarID, Action)> = Vec::new();
        for c in self.cars.values() {
            requested_moves.push((c.id, c.react(map, time, intersections)));
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
                Action::Continue(accel, ref maybe_request) => {
                    let c = self.cars.get_mut(&id).unwrap();
                    c.step_continue(accel, map, intersections);
                    // TODO maybe just return TurnID
                    if let Some(req) = maybe_request {
                        // Note this is idempotent and does NOT grant the request.
                        intersections.submit_request(req.clone(), time);
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
}
