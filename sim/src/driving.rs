use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap};
use dimensioned::si;
use draw_car::DrawCar;
use geom::EPSILON_DIST;
use intersections::{AgentInfo, IntersectionSimState, Request};
use kinematics;
use kinematics::Vehicle;
use map_model::geometry::LANE_THICKNESS;
use map_model::{LaneID, Map, TurnID};
use multimap::MultiMap;
use ordered_float::NotNaN;
use parking::{ParkingSimState, ParkingSpot};
use rand::Rng;
use sim::CarParking;
use std;
use std::collections::{BTreeMap, HashMap, VecDeque};
use {Acceleration, AgentID, CarID, CarState, Distance, InvariantViolated, On, Speed, Tick, Time};

const TIME_TO_PARK_OR_DEPART: Time = si::Second {
    value_unsafe: 10.0,
    _marker: std::marker::PhantomData,
};

#[derive(Clone, Serialize, Deserialize)]
struct ParkingState {
    // False means departing
    is_parking: bool,
    started_at: Tick,
    tuple: CarParking,
}

#[derive(Clone, Serialize, Deserialize)]
struct Car {
    id: CarID,
    on: On,
    speed: Speed,
    dist_along: Distance,

    parking: Option<ParkingState>,

    // TODO should this only be turns?
    waiting_for: Option<On>,
    // Head is the next lane
    path: VecDeque<LaneID>,

    debug: bool,
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
    StartParking(ParkingSpot),
    WorkOnParking,
    // True means we need to look for parking
    Continue(Acceleration, Vec<Request>, bool),
}

impl Car {
    fn find_parking_spot(
        &self,
        driving_lane: LaneID,
        dist_along: Distance,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) -> Option<ParkingSpot> {
        map.get_parent(driving_lane)
            .find_parking_lane(driving_lane)
            .and_then(|l| parking_sim.get_first_free_spot(l, dist_along))
    }

    // Note this doesn't change the car's state, and it observes a fixed view of the world!
    fn react(
        &self,
        map: &Map,
        time: Tick,
        sim: &DrivingSimState,
        parking_sim: &ParkingSimState,
        intersections: &IntersectionSimState,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Action {
        if self.parking.is_some() {
            // TODO right place for this check?
            assert!(self.speed <= kinematics::EPSILON_SPEED);
            return Action::WorkOnParking;
        }

        let vehicle = &properties[&self.id];

        if self.path.is_empty() && self.speed <= kinematics::EPSILON_SPEED {
            if let Some(spot) =
                self.find_parking_spot(self.on.as_lane(), self.dist_along, map, parking_sim)
            {
                if spot.dist_along_for_car(vehicle) == self.dist_along {
                    return Action::StartParking(spot);
                }
                // Being stopped before the parking spot is normal if the final road is clogged
                // with other drivers.
            }
        }

        // TODO could wrap this state up
        let mut current_speed_limit = self.on.speed_limit(map);
        let mut dist_to_lookahead = vehicle.max_lookahead_dist(self.speed, current_speed_limit)
            + Vehicle::worst_case_following_dist();
        // TODO when we add stuff here, optionally log stuff?
        let mut constraints: Vec<Acceleration> = Vec::new();
        let mut requests: Vec<Request> = Vec::new();
        let mut need_parking = false;
        let mut current_on = self.on;
        let mut current_dist_along = self.dist_along;
        let mut current_path = self.path.clone();
        let mut dist_scanned_ahead = 0.0 * si::M;

        loop {
            if self.debug {
                println!(
                    "  -- {} looking ahead to {:?} with {} left to scan",
                    self.id, current_on, dist_to_lookahead
                );
            }

            // Don't exceed the speed limit
            {
                let accel =
                    vehicle.accel_to_achieve_speed_in_one_tick(self.speed, current_speed_limit);
                constraints.push(accel);
                if self.debug {
                    println!("  {} needs {} to match speed limit", self.id, accel);
                }
            }

            // Don't hit the vehicle in front of us
            if let Some(other) = sim.next_car_in_front_of(current_on, current_dist_along) {
                assert!(self != other);
                assert!(current_dist_along < other.dist_along);
                let other_vehicle = &properties[&other.id];
                let dist_behind_other =
                    dist_scanned_ahead + (other.dist_along - current_dist_along);
                // If our lookahead doesn't even hit the lead vehicle (plus following distance!!!), then ignore them.
                if dist_to_lookahead + other_vehicle.following_dist() >= dist_behind_other {
                    let accel = vehicle.accel_to_follow(
                        self.speed,
                        current_speed_limit,
                        other_vehicle,
                        dist_behind_other,
                        other.speed,
                    );

                    if self.debug {
                        println!(
                            "  {} needs {} to not hit {}. Currently {} behind them",
                            self.id, accel, other.id, dist_behind_other
                        );
                    }

                    constraints.push(accel);
                }
            }

            // Stop for intersections or a parking spot?
            if let On::Lane(id) = current_on {
                let dist_to_maybe_stop_at = if current_path.is_empty() {
                    if let Some(spot) =
                        self.find_parking_spot(id, current_dist_along, map, parking_sim)
                    {
                        spot.dist_along_for_car(vehicle)
                    } else {
                        need_parking = true;
                        current_on.length(map)
                    }
                } else {
                    current_on.length(map)
                };
                let dist_from_stop = dist_to_maybe_stop_at - current_dist_along;

                // If our lookahead doesn't even hit the intersection / parking spot, then ignore
                // it. This means we won't request turns until we're close.
                if dist_to_lookahead >= dist_from_stop {
                    let should_stop = if current_path.is_empty() {
                        true
                    } else {
                        let req = Request::for_car(self.id, choose_turn(&current_path, id, map));
                        let granted = intersections.request_granted(req.clone());
                        if !granted {
                            // Otherwise, we wind up submitting a request at the end of our step, after
                            // we've passed through the intersection!
                            requests.push(req);
                        }
                        !granted
                    };
                    if should_stop {
                        let accel = vehicle.accel_to_stop_in_dist(self.speed, dist_from_stop);
                        if self.debug {
                            println!("  {} needs {} to stop for the intersection or parking spot that's currently {} away", self.id, accel, dist_from_stop);
                        }
                        constraints.push(accel);
                        // No use in further lookahead.
                        break;
                    }
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
                On::Lane(l) => On::Turn(choose_turn(&current_path, l, map)),
            };
            current_speed_limit = current_on.speed_limit(map);
            current_dist_along = 0.0 * si::M;
            dist_scanned_ahead += dist_this_step;
        }

        // Clamp based on what we can actually do
        // TODO this type mangling is awful
        let safe_accel = vehicle.clamp_accel(
            constraints
                .into_iter()
                .min_by_key(|a| NotNaN::new(a.value_unsafe).unwrap())
                .unwrap(),
        );
        if self.debug {
            println!(
                "At {}, {} chose {}, with current speed {}",
                time, self.id, safe_accel, self.speed
            );
        }

        Action::Continue(safe_accel, requests, need_parking)
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
            if leftover_dist <= EPSILON_DIST {
                break;
            }
            let next_on = match self.on {
                On::Turn(t) => On::Lane(map.get_t(t).dst),
                On::Lane(l) => On::Turn(choose_turn(&self.path, l, map)),
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

    // Return true if we're just plain stuck :(
    fn look_for_parking<R: Rng + ?Sized>(&mut self, map: &Map, rng: &mut R) -> bool {
        let last_lane = if self.path.is_empty() {
            match self.on {
                On::Turn(t) => t.dst,
                On::Lane(l) => l,
            }
        } else {
            *self.path.back().unwrap()
        };

        // TODO Better strategies than random: look for lanes with free spots (if it'd be feasible
        // to physically see the spots), stay close to the original goal, avoid lanes we've
        // visited, prefer easier turns...
        let choices = map.get_next_lanes(last_lane);
        if choices.is_empty() {
            if self.debug {
                println!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever", self.id, last_lane);
            }
            return true;
        }
        let choice = rng.choose(&choices).unwrap().id;
        if self.debug {
            println!(
                "{} can't find parking on {}, so wandering over to {}",
                self.id, last_lane, choice
            );
        }
        self.path.push_back(choice);
        false
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
            capacity: ((id.length(map) / Vehicle::best_case_following_dist()).ceil() as usize)
                .max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    fn reset(
        &mut self,
        ids: &Vec<CarID>,
        cars: &BTreeMap<CarID, Car>,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Result<(), InvariantViolated> {
        let old_queue = self.cars_queue.clone();

        if ids.len() > self.capacity {
            let dists: Vec<Distance> = ids.iter().map(|id| cars[id].dist_along).collect();
            return Err(InvariantViolated(format!(
                "on {:?}, reset to {:?} broke, because capacity is just {}. dist_alongs are {:?}",
                self.id, ids, self.capacity, dists
            )));
        }
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        // Sort descending.
        self.cars_queue
            .sort_by_key(|id| -NotNaN::new(cars[id].dist_along.value_unsafe).unwrap());

        // assert here we're not squished together too much
        for slice in self.cars_queue.windows(2) {
            let (c1, c2) = (slice[0], slice[1]);
            let following_dist = properties[&c1].following_dist();
            let (dist1, dist2) = (cars[&c1].dist_along, cars[&c2].dist_along);
            if dist1 - dist2 < following_dist {
                return Err(InvariantViolated(format!("uh oh! on {:?}, reset to {:?} broke. min following distance is {}, but we have {} at {} and {} at {}. dist btwn is just {}. prev queue was {:?}", self.id, self.cars_queue, following_dist, c1, dist1, c2, dist2, dist1 - dist2, old_queue)));
            }
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    // TODO this starts cars with their front aligned with the end of the lane, sticking their back
    // into the intersection. :(
    fn get_draw_cars(
        &self,
        sim: &DrivingSimState,
        map: &Map,
        time: Tick,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Vec<DrawCar> {
        let mut results = Vec::new();
        for id in &self.cars_queue {
            results.push(sim.get_draw_car(*id, time, map, properties).unwrap())
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

    fn first_car_behind(&self, dist: Distance, sim: &DrivingSimState) -> Option<CarID> {
        self.cars_queue
            .iter()
            .find(|id| sim.cars[id].dist_along <= dist)
            .map(|id| *id)
    }

    fn insert_at(
        &mut self,
        car: CarID,
        dist_along: Distance,
        dist_per_car: HashMap<CarID, Distance>,
    ) {
        if let Some(idx) = self.cars_queue
            .iter()
            .position(|id| dist_per_car[id] < dist_along)
        {
            self.cars_queue.insert(idx, car);
        } else {
            self.cars_queue.push(car);
        }
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

    pub fn populate_info_for_intersections(&self, info: &mut AgentInfo, _map: &Map) {
        for c in self.cars.values() {
            let id = AgentID::Car(c.id);
            info.speeds.insert(id, c.speed);
            if self.next_car_in_front_of(c.on, c.dist_along).is_none() {
                info.leaders.insert(id);
            }
        }
    }

    pub fn get_car_state(&self, c: CarID) -> CarState {
        if let Some(driving) = self.cars.get(&c) {
            if driving.speed > kinematics::EPSILON_SPEED {
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
            .filter(|c| c.speed <= kinematics::EPSILON_SPEED)
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

    pub fn step<R: Rng + ?Sized>(
        &mut self,
        time: Tick,
        map: &Map,
        // TODO not all of it, just for one query!
        parking_sim: &ParkingSimState,
        intersections: &mut IntersectionSimState,
        rng: &mut R,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Result<Vec<CarParking>, InvariantViolated> {
        // Could be concurrent, since this is deterministic.
        let mut requested_moves: Vec<(CarID, Action)> = Vec::new();
        for c in self.cars.values() {
            requested_moves.push((
                c.id,
                c.react(map, time, self, parking_sim, intersections, properties),
            ));
        }

        // In AORTA, there was a split here -- react vs step phase. We're still following the same
        // thing, but it might be slightly more clear to express it differently?

        let mut finished_parking: Vec<CarParking> = Vec::new();

        // Apply moves. This should resolve in no conflicts because lookahead behavior works, so
        // this could be applied concurrently!
        for (id, act) in &requested_moves {
            match *act {
                Action::StartParking(ref spot) => {
                    let c = self.cars.get_mut(&id).unwrap();
                    c.parking = Some(ParkingState {
                        is_parking: true,
                        started_at: time,
                        tuple: CarParking::new(*id, spot.clone()),
                    });
                }
                Action::WorkOnParking => {
                    let state = self.cars.get_mut(&id).unwrap().parking.take().unwrap();
                    if state.started_at + TIME_TO_PARK_OR_DEPART == time {
                        if state.is_parking {
                            finished_parking.push(state.tuple);
                            // No longer need to represent the car in the driving state
                            self.cars.remove(&id);
                        }
                    } else {
                        self.cars.get_mut(&id).unwrap().parking = Some(state);
                    }
                }
                Action::Continue(accel, ref requests, need_parking) => {
                    let should_remove = {
                        let c = self.cars.get_mut(&id).unwrap();
                        c.step_continue(accel, map, intersections)?;
                        // TODO maybe just return TurnID
                        for req in requests {
                            // Note this is idempotent and does NOT grant the request.
                            // TODO should we check that the car is currently the lead vehicle?
                            // intersection is assuming that! or relax that assumption.
                            intersections.submit_request(req.clone())?;

                            // TODO kind of a weird way to figure out when to fill this out...
                            // duplicated with stop sign's check, also. should check that they're a
                            // leader vehicle...
                            if On::Lane(req.turn.src) == c.on
                                && c.speed <= kinematics::EPSILON_SPEED
                            {
                                c.waiting_for = Some(On::Turn(req.turn));
                            }
                        }
                        need_parking && c.look_for_parking(map, rng)
                    };
                    if should_remove {
                        self.cars.remove(&id);
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
                l.reset(v, &self.cars, properties)?;
            } else {
                l.reset(&Vec::new(), &self.cars, properties)?;
            }
        }
        for t in self.turns.values_mut() {
            if let Some(v) = cars_per_turn.get_vec(&t.id.as_turn()) {
                t.reset(v, &self.cars, properties)?;
            } else {
                t.reset(&Vec::new(), &self.cars, properties)?;
            }
        }

        Ok(finished_parking)
    }

    // True if the car started, false if there wasn't currently room
    pub fn start_car_on_lane(
        &mut self,
        time: Tick,
        car: CarID,
        parking: CarParking,
        mut path: VecDeque<LaneID>,
        map: &Map,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> bool {
        let vehicle = &properties[&car];
        let start = path.pop_front().unwrap();
        // TODO this looks like it jumps when the parking and driving lanes are different lengths
        // due to diagonals
        let dist_along = parking.spot.dist_along_for_car(vehicle);
        // If not, we have a parking lane much longer than a driving lane...
        assert!(dist_along <= map.get_l(start).length());

        // Is it safe to enter the lane right now? Start scanning ahead of where we'll enter, so we
        // don't hit somebody's back
        if let Some(other) = self.lanes[start.0]
            .first_car_behind(dist_along + Vehicle::worst_case_following_dist(), self)
        {
            let other_dist = self.cars[&other].dist_along;
            if other_dist >= dist_along {
                println!(
                    "{} can't spawn, because they'd wind up too close ({}) behind {}",
                    car,
                    other_dist - dist_along,
                    other
                );
                return false;
            }

            let accel_for_other_to_stop = vehicle.accel_to_follow(
                self.cars[&other].speed,
                map.get_parent(start).get_speed_limit(),
                &properties[&other],
                dist_along - other_dist,
                0.0 * si::MPS,
            );
            if accel_for_other_to_stop <= vehicle.max_deaccel {
                println!("{} can't spawn {} in front of {}, because {} would have to do {} to not hit {}", car, dist_along - other_dist, other, other, accel_for_other_to_stop, car);
                return false;
            }

            // TODO check that there's not a car elsewhere that's about to wind up here. can check
            // the intersection for accepted turns to this lane. or, enforce that no parking spots
            // can exist before the worst-case entry distance (based on the speed limit).
        }

        self.cars.insert(
            car,
            Car {
                id: car,
                path,
                dist_along: dist_along,
                speed: 0.0 * si::MPS,
                on: On::Lane(start),
                waiting_for: None,
                debug: false,
                parking: Some(ParkingState {
                    is_parking: false,
                    started_at: time,
                    tuple: parking,
                }),
            },
        );
        let mut dist_per_car: HashMap<CarID, Distance> = HashMap::new();
        for c in &self.lanes[start.0].cars_queue {
            dist_per_car.insert(*c, self.cars[&c].dist_along);
        }
        self.lanes[start.0].insert_at(car, dist_along, dist_per_car);
        true
    }

    pub fn get_draw_car(
        &self,
        id: CarID,
        time: Tick,
        map: &Map,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Option<DrawCar> {
        let c = self.cars.get(&id)?;
        let (base_pos, angle) = c.on.dist_along(c.dist_along, map);
        let vehicle = &properties[&id];
        let stopping_dist = vehicle.stopping_distance(c.speed);

        // TODO arguably, this math might belong in DrawCar.
        let pos = if let Some(ref parking) = c.parking {
            let progress: f64 =
                ((time - parking.started_at).as_time() / TIME_TO_PARK_OR_DEPART).value_unsafe;
            assert!(progress >= 0.0 && progress <= 1.0);
            let project_away_ratio = if parking.is_parking {
                progress
            } else {
                1.0 - progress
            };
            // TODO we're assuming the parking lane is to the right of us!
            base_pos.project_away(project_away_ratio * LANE_THICKNESS, angle.rotate_degs(90.0))
        } else {
            base_pos
        };

        Some(DrawCar::new(
            c.id,
            vehicle,
            c.waiting_for.and_then(|on| on.maybe_turn()),
            map,
            pos,
            angle,
            stopping_dist,
        ))
    }

    pub fn get_draw_cars_on_lane(
        &self,
        lane: LaneID,
        time: Tick,
        map: &Map,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Vec<DrawCar> {
        self.lanes[lane.0].get_draw_cars(self, map, time, properties)
    }

    pub fn get_draw_cars_on_turn(
        &self,
        turn: TurnID,
        time: Tick,
        map: &Map,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Vec<DrawCar> {
        if let Some(queue) = self.turns.get(&turn) {
            return queue.get_draw_cars(self, map, time, properties);
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

fn choose_turn(path: &VecDeque<LaneID>, from: LaneID, map: &Map) -> TurnID {
    for t in map.get_turns_from_lane(from) {
        if t.dst == path[0] {
            return t.id;
        }
    }
    panic!("No turn from {} to {}", from, path[0]);
}
