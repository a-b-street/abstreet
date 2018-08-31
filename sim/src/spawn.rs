use driving::DrivingSimState;
use kinematics::Vehicle;
use map_model;
use map_model::{BuildingID, BusStop, LaneID, Map};
use parking::ParkingSimState;
use rand::Rng;
use router::Router;
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;
use transit::TransitSimState;
use trips::TripManager;
use walking::{SidewalkSpot, WalkingSimState};
use {AgentID, CarID, Event, ParkedCar, ParkingSpot, PedestrianID, Tick, TripID};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
enum Command {
    Walk(Tick, TripID, PedestrianID, SidewalkSpot, SidewalkSpot),
    Drive(Tick, TripID, ParkedCar, BuildingID),
}

impl Command {
    fn at(&self) -> Tick {
        match self {
            Command::Walk(at, _, _, _, _) => *at,
            Command::Drive(at, _, _, _) => *at,
        }
    }

    fn get_pathfinding_lanes(&self, map: &Map) -> (LaneID, LaneID) {
        match self {
            Command::Walk(_, _, _, spot1, spot2) => (spot1.sidewalk, spot2.sidewalk),
            Command::Drive(_, _, parked_car, goal_bldg) => (
                map.get_driving_lane_from_parking(parked_car.spot.lane)
                    .unwrap(),
                map.get_driving_lane_from_bldg(*goal_bldg).unwrap(),
            ),
        }
    }

    fn retry_next_tick(&self) -> Command {
        match self {
            Command::Walk(at, trip, ped, spot1, spot2) => {
                Command::Walk(at.next(), *trip, *ped, spot1.clone(), spot2.clone())
            }
            Command::Drive(at, trip, parked_car, goal) => {
                Command::Drive(at.next(), *trip, parked_car.clone(), *goal)
            }
        }
    }
}

// This owns car/ped IDs.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Spawner {
    // Ordered by time
    commands: VecDeque<Command>,

    car_id_counter: usize,
    ped_id_counter: usize,

    // No real use in being owned by Sim. All of the transitions will need us to spawn something.
    // TODO next step would be moving all of the methods that create trips (choosing random stuff
    // or not) out into something else, probably helpers.
    trips: TripManager,
}

impl Spawner {
    pub fn empty() -> Spawner {
        Spawner {
            commands: VecDeque::new(),
            car_id_counter: 0,
            ped_id_counter: 0,
            trips: TripManager::new(),
        }
    }

    pub fn step(
        &mut self,
        events: &mut Vec<Event>,
        now: Tick,
        map: &Map,
        parking_sim: &mut ParkingSimState,
        walking_sim: &mut WalkingSimState,
        driving_sim: &mut DrivingSimState,
        properties: &BTreeMap<CarID, Vehicle>,
    ) {
        let mut commands: Vec<Command> = Vec::new();
        let mut requested_paths: Vec<(LaneID, LaneID)> = Vec::new();
        loop {
            if self.commands
                .front()
                .and_then(|cmd| Some(now == cmd.at()))
                .unwrap_or(false)
            {
                let cmd = self.commands.pop_front().unwrap();
                requested_paths.push(cmd.get_pathfinding_lanes(map));
                commands.push(cmd);
            } else {
                break;
            }
        }
        if commands.is_empty() {
            return;
        }
        let paths = calculate_paths(&requested_paths, map);

        let mut spawned_agents = 0;
        for (cmd, (req, maybe_path)) in commands.into_iter().zip(requested_paths.iter().zip(paths))
        {
            if let Some(path) = maybe_path {
                match cmd {
                    Command::Drive(_, trip, ref parked_car, goal_bldg) => {
                        let car = parked_car.car;

                        // TODO this looks like it jumps when the parking and driving lanes are different lengths
                        // due to diagonals
                        let dist_along =
                            parking_sim.dist_along_for_car(parked_car.spot, &properties[&car]);
                        let mut path_queue = VecDeque::from(path);
                        let start = path_queue.pop_front().unwrap();
                        if driving_sim.start_car_on_lane(
                            events,
                            now,
                            car,
                            Some(parked_car.clone()),
                            dist_along,
                            start,
                            Router::make_router_to_park(path_queue, goal_bldg),
                            map,
                            properties,
                        ) {
                            self.trips.agent_starting_trip_leg(AgentID::Car(car), trip);
                            parking_sim.remove_parked_car(parked_car.clone());
                            spawned_agents += 1;
                        } else {
                            // Try again next tick. Because we already slurped up all the commands
                            // for this tick, the front of the queue is the right spot.
                            self.commands.push_front(cmd.retry_next_tick());
                        }
                    }
                    Command::Walk(_, trip, ped, spot1, spot2) => {
                        self.trips
                            .agent_starting_trip_leg(AgentID::Pedestrian(ped), trip);
                        walking_sim.seed_pedestrian(
                            events,
                            ped,
                            spot1,
                            spot2,
                            map,
                            VecDeque::from(path),
                        );
                        spawned_agents += 1;
                    }
                };
            } else if false {
                println!(
                    "Couldn't find path from {} to {} for {:?}",
                    req.0, req.1, cmd
                );
            }
        }
        if false {
            println!(
                "Spawned {} agents of requested {}",
                spawned_agents,
                requested_paths.len()
            );
        }
    }

    // This happens immediately; it isn't scheduled.
    pub fn seed_bus_route<R: Rng + ?Sized>(
        &mut self,
        events: &mut Vec<Event>,
        stops: Vec<BusStop>,
        rng: &mut R,
        map: &Map,
        driving_sim: &mut DrivingSimState,
        transit_sim: &mut TransitSimState,
        now: Tick,
        properties: &BTreeMap<CarID, Vehicle>,
    ) -> Vec<Vehicle> {
        let route = transit_sim.create_empty_route(stops);
        let mut vehicles: Vec<Vehicle> = Vec::new();
        // Try to spawn a bus at each stop
        for (next_stop_idx, start_dist_along, mut path) in
            transit_sim.get_route_starts(route, map).into_iter()
        {
            let id = CarID(self.car_id_counter);
            self.car_id_counter += 1;
            let vehicle = Vehicle::generate_bus(id, rng);

            let start = path.pop_front().unwrap();
            if driving_sim.start_car_on_lane(
                events,
                now,
                id,
                None,
                start_dist_along,
                start,
                Router::make_router_for_bus(path),
                map,
                properties,
            ) {
                transit_sim.bus_created(id, route, next_stop_idx);
                println!("Spawned bus {} for route {}", id, route);
                vehicles.push(vehicle);
            } else {
                println!(
                    "No room for a bus headed towards stop {} of {}, giving up",
                    next_stop_idx, route
                );
            }
        }
        vehicles
    }

    // This happens immediately; it isn't scheduled.
    pub fn seed_parked_cars<R: Rng + ?Sized>(
        &mut self,
        percent_capacity_to_fill: f64,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
    ) -> Vec<Vehicle> {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);

        let mut total_capacity = 0;
        let mut new_cars: Vec<Vehicle> = Vec::new();
        for spot in parking_sim.get_all_free_spots() {
            total_capacity += 1;
            if rng.gen_bool(percent_capacity_to_fill) {
                let car = CarID(self.car_id_counter);
                // TODO since spawning applies during the next step, lots of stuff breaks without
                // this :(
                parking_sim.add_parked_car(ParkedCar::new(car, spot));
                new_cars.push(Vehicle::generate_typical_car(car, rng));
                self.car_id_counter += 1;
            }
        }

        println!(
            "Seeded {} of {} parking spots with cars",
            new_cars.len(),
            total_capacity
        );
        new_cars
    }

    pub fn seed_specific_parked_cars<R: Rng + ?Sized>(
        &mut self,
        lane: LaneID,
        spot_indices: Vec<usize>,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
    ) -> Vec<Vehicle> {
        let spots = parking_sim.get_all_spots(lane);
        spot_indices
            .into_iter()
            .map(|idx| {
                let car = CarID(self.car_id_counter);
                parking_sim.add_parked_car(ParkedCar::new(car, spots[idx]));
                self.car_id_counter += 1;
                Vehicle::generate_typical_car(car, rng)
            })
            .collect()
    }

    pub fn start_parked_car_with_goal<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        car: CarID,
        parking_sim: &ParkingSimState,
        goal: LaneID,
        rng: &mut R,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.at());
        }

        // Don't add duplicate commands.
        if let Some(trip) = self.trips.get_trip_using_car(car) {
            println!(
                "{} is already a part of {}, ignoring new request",
                car, trip
            );
            return;
        }

        let parking_lane = parking_sim.lane_of_car(car).expect("Car isn't parked");
        let road = map.get_parent(parking_lane);
        let sidewalk = road.find_sidewalk(parking_lane)
            .expect("Parking lane has no sidewalk");
        let start_bldg = pick_bldg_from_sidewalk(rng, map, sidewalk);
        let goal_bldg = pick_bldg_from_driving_lane(rng, map, goal);

        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        self.commands.push_back(Command::Walk(
            at,
            self.trips
                .new_trip(ped_id, start_bldg, Some(car), goal_bldg),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            SidewalkSpot::parking_spot(
                parking_sim.get_spot_of_car(car, parking_lane),
                map,
                parking_sim,
            ),
        ));
    }

    pub fn start_parked_car<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        car: CarID,
        parking_sim: &ParkingSimState,
        rng: &mut R,
    ) {
        let parking_lane = parking_sim.lane_of_car(car).expect("Car isn't parked");
        let road = map.get_parent(parking_lane);
        let driving_lane = road.find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        let goal = pick_car_goal(rng, map, driving_lane);
        self.start_parked_car_with_goal(at, map, car, parking_sim, goal, rng);
    }

    pub fn seed_driving_trips<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        num: usize,
        rng: &mut R,
        parking_sim: &ParkingSimState,
    ) {
        let mut cars: Vec<CarID> = parking_sim
            .get_all_parked_cars()
            .into_iter()
            .filter_map(|parked_car| {
                let lane = parked_car.spot.lane;
                let has_bldgs = map.get_parent(lane)
                    .find_sidewalk(lane)
                    .and_then(|sidewalk| Some(!map.get_l(sidewalk).building_paths.is_empty()))
                    .unwrap_or(false);
                if has_bldgs {
                    map.get_parent(lane)
                        .find_driving_lane(lane)
                        .and_then(|_driving_lane| Some(parked_car.car))
                } else {
                    None
                }
            })
            .collect();
        if cars.is_empty() {
            return;
        }
        rng.shuffle(&mut cars);

        for car in &cars[0..num.min(cars.len())] {
            self.start_parked_car(at, map, *car, parking_sim, rng);
        }
    }

    pub fn spawn_specific_pedestrian(
        &mut self,
        at: Tick,
        map: &Map,
        start_bldg: BuildingID,
        goal_bldg: BuildingID,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.at());
        }

        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        self.commands.push_back(Command::Walk(
            at,
            self.trips.new_trip(ped_id, start_bldg, None, goal_bldg),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            SidewalkSpot::building(goal_bldg, map),
        ));
    }

    pub fn spawn_pedestrian<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        sidewalk: LaneID,
        rng: &mut R,
    ) {
        assert!(map.get_l(sidewalk).is_sidewalk());
        let start_bldg = pick_bldg_from_sidewalk(rng, map, sidewalk);
        let goal_bldg = pick_ped_goal(rng, map, sidewalk);
        self.spawn_specific_pedestrian(at, map, start_bldg, goal_bldg);
    }

    pub fn seed_walking_trips<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        num: usize,
        rng: &mut R,
    ) {
        let mut sidewalks: Vec<LaneID> = Vec::new();
        for l in map.all_lanes() {
            if l.is_sidewalk() && !l.building_paths.is_empty() {
                sidewalks.push(l.id);
            }
        }

        for _i in 0..num {
            let start = *rng.choose(&sidewalks).unwrap();
            self.spawn_pedestrian(at, map, start, rng);
        }
    }

    // Trip transitions
    pub fn car_reached_parking_spot(
        &mut self,
        at: Tick,
        p: ParkedCar,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) {
        let (trip, ped, goal_bldg) = self.trips.car_reached_parking_spot(p.car);
        self.commands.push_back(Command::Walk(
            at.next(),
            trip,
            ped,
            SidewalkSpot::parking_spot(p.spot, map, parking_sim),
            SidewalkSpot::building(goal_bldg, map),
        ));
    }

    pub fn ped_reached_parking_spot(
        &mut self,
        at: Tick,
        ped: PedestrianID,
        spot: ParkingSpot,
        parking_sim: &ParkingSimState,
    ) {
        let (trip, goal_bldg) = self.trips.ped_reached_parking_spot(ped);
        self.commands.push_back(Command::Drive(
            at.next(),
            trip,
            parking_sim.get_car_at_spot(spot).unwrap(),
            goal_bldg,
        ));
    }

    pub fn is_done(&self) -> bool {
        self.commands.is_empty()
    }
}

fn pick_car_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> LaneID {
    let candidate_goals: Vec<LaneID> = map.all_lanes()
        .iter()
        .filter_map(|l| {
            if l.id != start && l.is_driving() {
                if let Some(sidewalk) = map.get_sidewalk_from_driving_lane(l.id) {
                    if !map.get_l(sidewalk).building_paths.is_empty() {
                        return Some(l.id);
                    }
                }
            }
            None
        })
        .collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn pick_ped_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> BuildingID {
    let candidate_goals: Vec<BuildingID> = map.all_buildings()
        .iter()
        .filter_map(|b| {
            if b.front_path.sidewalk != start {
                Some(b.id)
            } else {
                None
            }
        })
        .collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn calculate_paths(requested_paths: &Vec<(LaneID, LaneID)>, map: &Map) -> Vec<Option<Vec<LaneID>>> {
    use rayon::prelude::*;

    if false {
        println!("Calculating {} paths", requested_paths.len())
    };
    // TODO better timer macro
    let timer = Instant::now();
    let paths: Vec<Option<Vec<LaneID>>> = requested_paths
        .par_iter()
        .map(|(start, goal)| map_model::pathfind(map, *start, *goal))
        .collect();

    let elapsed = timer.elapsed();
    let dt = elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) * 1e-9;
    if false {
        println!("Calculating {} paths took {}s", paths.len(), dt)
    };
    paths
}

fn pick_bldg_from_sidewalk<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    sidewalk: LaneID,
) -> BuildingID {
    *rng.choose(&map.get_l(sidewalk).building_paths)
        .expect(&format!("{} has no buildings", sidewalk))
}

fn pick_bldg_from_driving_lane<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    start: LaneID,
) -> BuildingID {
    pick_bldg_from_sidewalk(rng, map, map.get_sidewalk_from_driving_lane(start).unwrap())
}
