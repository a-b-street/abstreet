use abstutil::{fork_rng, WeightedUsizeChoice};
use dimensioned::si;
use driving::{CreateCar, DrivingGoal, DrivingSimState};
use kinematics::Vehicle;
use map_model::{
    BuildingID, BusRoute, BusStopID, LaneID, LaneType, Map, Path, PathRequest, Pathfinder, RoadID,
};
use parking::ParkingSimState;
use rand::{Rng, XorShiftRng};
use router::Router;
use scheduler;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use transit::TransitSimState;
use trips::{TripLeg, TripManager};
use walking::{CreatePedestrian, SidewalkSpot};
use {CarID, Distance, Event, ParkedCar, ParkingSpot, PedestrianID, Tick, TripID, VehicleType};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
enum Command {
    Walk(Tick, TripID, PedestrianID, SidewalkSpot, SidewalkSpot),
    Drive(Tick, TripID, ParkedCar, DrivingGoal),
    DriveFromBorder {
        at: Tick,
        trip: TripID,
        car: CarID,
        vehicle: Vehicle,
        start: LaneID,
        goal: DrivingGoal,
    },
    Bike {
        at: Tick,
        trip: TripID,
        start_sidewalk: LaneID,
        start_dist: Distance,
        vehicle: Vehicle,
        goal: DrivingGoal,
    },
}

impl Command {
    fn at(&self) -> Tick {
        match self {
            Command::Walk(at, _, _, _, _) => *at,
            Command::Drive(at, _, _, _) => *at,
            Command::DriveFromBorder { at, .. } => *at,
            Command::Bike { at, .. } => *at,
        }
    }

    fn get_pathfinding_request(&self, map: &Map, parking_sim: &ParkingSimState) -> PathRequest {
        match self {
            Command::Walk(_, _, _, start, goal) => PathRequest {
                start: start.sidewalk,
                start_dist: start.dist_along,
                end: goal.sidewalk,
                end_dist: goal.dist_along,
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            Command::Drive(_, _, parked_car, goal) => {
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => find_driving_lane_near_building(*b, map),
                    DrivingGoal::Border(_, l) => *l,
                };
                PathRequest {
                    start: map
                        .get_driving_lane_from_parking(parked_car.spot.lane)
                        .unwrap(),
                    start_dist: parking_sim
                        .dist_along_for_car(parked_car.spot, &parked_car.vehicle),
                    end: goal_lane,
                    end_dist: map.get_l(goal_lane).length(),
                    can_use_bike_lanes: false,
                    can_use_bus_lanes: false,
                }
            }
            Command::Bike {
                start_sidewalk,
                start_dist,
                goal,
                ..
            } => {
                let (goal_lane, goal_dist) = match goal {
                    DrivingGoal::ParkNear(b) => find_biking_goal_near_building(*b, map),
                    DrivingGoal::Border(_, l) => (*l, map.get_l(*l).length()),
                };
                PathRequest {
                    // TODO or bike lane, gah
                    start: map.get_driving_lane_from_sidewalk(*start_sidewalk).unwrap(),
                    start_dist: *start_dist,
                    end: goal_lane,
                    end_dist: goal_dist,
                    can_use_bus_lanes: false,
                    can_use_bike_lanes: true,
                }
            }
            Command::DriveFromBorder {
                start,
                goal,
                vehicle,
                ..
            } => {
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => find_driving_lane_near_building(*b, map),
                    DrivingGoal::Border(_, l) => *l,
                };
                PathRequest {
                    start: *start,
                    start_dist: 0.0 * si::M,
                    end: goal_lane,
                    end_dist: map.get_l(goal_lane).length(),
                    can_use_bus_lanes: vehicle.vehicle_type == VehicleType::Bus,
                    can_use_bike_lanes: vehicle.vehicle_type == VehicleType::Bike,
                }
            }
        }
    }
}

// This owns car/ped IDs.
#[derive(Serialize, Deserialize, PartialEq)]
pub struct Spawner {
    // Ordered descending by time
    commands: Vec<Command>,

    car_id_counter: usize,
    ped_id_counter: usize,
}

impl Spawner {
    pub fn empty() -> Spawner {
        Spawner {
            commands: Vec::new(),
            car_id_counter: 0,
            ped_id_counter: 0,
        }
    }

    pub fn step(
        &mut self,
        now: Tick,
        map: &Map,
        scheduler: &mut scheduler::Scheduler,
        parking_sim: &mut ParkingSimState,
    ) {
        let mut commands: Vec<Command> = Vec::new();
        let mut requests: Vec<PathRequest> = Vec::new();
        loop {
            if self
                .commands
                .last()
                .and_then(|cmd| Some(now == cmd.at()))
                .unwrap_or(false)
            {
                let cmd = self.commands.pop().unwrap();
                requests.push(cmd.get_pathfinding_request(map, parking_sim));
                commands.push(cmd);
            } else {
                break;
            }
        }
        if commands.is_empty() {
            return;
        }
        let paths = calculate_paths(map, &requests);

        for (cmd, (req, maybe_path)) in commands.into_iter().zip(requests.iter().zip(paths)) {
            if let Some(path) = maybe_path {
                match cmd {
                    Command::Drive(_, trip, ref parked_car, ref goal) => {
                        let car = parked_car.car;

                        // TODO this looks like it jumps when the parking and driving lanes are different lengths
                        // due to diagonals
                        let dist_along =
                            parking_sim.dist_along_for_car(parked_car.spot, &parked_car.vehicle);
                        let start = path.current_step().as_traversable().as_lane();

                        scheduler.enqueue_command(scheduler::Command::SpawnCar(
                            now,
                            CreateCar {
                                car,
                                trip: Some(trip),
                                owner: parked_car.owner,
                                maybe_parked_car: Some(parked_car.clone()),
                                vehicle: parked_car.vehicle.clone(),
                                start,
                                dist_along,
                                router: match goal {
                                    DrivingGoal::ParkNear(b) => {
                                        Router::make_router_to_park(path, *b)
                                    }
                                    DrivingGoal::Border(_, _) => {
                                        Router::make_router_to_border(path)
                                    }
                                },
                            },
                        ));
                    }
                    Command::DriveFromBorder {
                        trip,
                        car,
                        ref vehicle,
                        start,
                        ref goal,
                        ..
                    } => {
                        scheduler.enqueue_command(scheduler::Command::SpawnCar(
                            now,
                            CreateCar {
                                car,
                                trip: Some(trip),
                                // TODO need a way to specify this in the scenario
                                owner: None,
                                maybe_parked_car: None,
                                vehicle: vehicle.clone(),
                                start,
                                dist_along: 0.0 * si::M,
                                router: match goal {
                                    DrivingGoal::ParkNear(b) => {
                                        Router::make_router_to_park(path, *b)
                                    }
                                    DrivingGoal::Border(_, _) => {
                                        Router::make_router_to_border(path)
                                    }
                                },
                            },
                        ));
                    }
                    Command::Bike {
                        trip,
                        ref vehicle,
                        ref goal,
                        ..
                    } => {
                        scheduler.enqueue_command(scheduler::Command::SpawnCar(
                            now,
                            CreateCar {
                                car: vehicle.id,
                                trip: Some(trip),
                                owner: None,
                                maybe_parked_car: None,
                                vehicle: vehicle.clone(),
                                start: req.start,
                                dist_along: req.start_dist,
                                router: match goal {
                                    DrivingGoal::ParkNear(_) => {
                                        Router::make_bike_router(path, req.end_dist)
                                    }
                                    DrivingGoal::Border(_, _) => {
                                        Router::make_router_to_border(path)
                                    }
                                },
                            },
                        ));
                    }
                    Command::Walk(_, trip, ped, start, goal) => {
                        scheduler.enqueue_command(scheduler::Command::SpawnPed(
                            now,
                            CreatePedestrian {
                                id: ped,
                                trip,
                                start,
                                goal,
                                path,
                            },
                        ));
                    }
                };
            } else {
                error!(
                    "Couldn't find path from {} to {} for {:?}",
                    req.start, req.end, cmd
                );
            }
        }
    }

    // This happens immediately; it isn't scheduled.
    pub fn seed_bus_route(
        &mut self,
        events: &mut Vec<Event>,
        route: &BusRoute,
        rng: &mut XorShiftRng,
        map: &Map,
        driving_sim: &mut DrivingSimState,
        transit_sim: &mut TransitSimState,
        now: Tick,
    ) -> Vec<CarID> {
        let route_id = transit_sim.create_empty_route(route, map);
        let mut results: Vec<CarID> = Vec::new();
        // Try to spawn a bus at each stop
        for (next_stop_idx, start_dist_along, path) in
            transit_sim.get_route_starts(route_id, map).into_iter()
        {
            let id = CarID(self.car_id_counter);
            self.car_id_counter += 1;
            let vehicle = Vehicle::generate_bus(id, rng);

            let start = path.current_step().as_traversable().as_lane();

            // TODO For now, skip spawning this bus too. :\
            if start_dist_along > map.get_l(start).length() {
                warn!(
                    "Bus stop is too far past equivalent driving lane; can't make a bus headed towards stop {} of {} ({})",
                    next_stop_idx, route.name, route_id
                );
                continue;
            }

            if driving_sim.start_car_on_lane(
                events,
                now,
                map,
                CreateCar {
                    car: id,
                    trip: None,
                    owner: None,
                    maybe_parked_car: None,
                    vehicle,
                    start,
                    dist_along: start_dist_along,
                    router: Router::make_router_for_bus(path),
                },
            ) {
                transit_sim.bus_created(id, route_id, next_stop_idx);
                info!("Spawned bus {} for route {} ({})", id, route.name, route_id);
                results.push(id);
            } else {
                warn!(
                    "No room for a bus headed towards stop {} of {} ({}), giving up",
                    next_stop_idx, route.name, route_id
                );
            }
        }
        results
    }

    // This happens immediately; it isn't scheduled.
    pub fn seed_parked_cars(
        &mut self,
        cars_per_building: &WeightedUsizeChoice,
        owner_buildings: &Vec<BuildingID>,
        neighborhoods_roads: &BTreeSet<RoadID>,
        parking_sim: &mut ParkingSimState,
        base_rng: &mut XorShiftRng,
        map: &Map,
    ) {
        // Track the available parking spots per road, only for the roads in the appropriate
        // neighborhood.
        let mut total_spots = 0;
        let mut open_spots_per_road: HashMap<RoadID, Vec<ParkingSpot>> = HashMap::new();
        for id in neighborhoods_roads {
            let r = map.get_r(*id);
            let mut spots: Vec<ParkingSpot> = Vec::new();
            for (lane, lane_type) in r
                .children_forwards
                .iter()
                .chain(r.children_backwards.iter())
            {
                if *lane_type == LaneType::Parking {
                    spots.extend(parking_sim.get_free_spots(*lane));
                }
            }
            total_spots += spots.len();
            fork_rng(base_rng).shuffle(&mut spots);
            open_spots_per_road.insert(r.id, spots);
        }

        let mut new_cars = 0;
        for b in owner_buildings {
            for _ in 0..cars_per_building.sample(base_rng) {
                let mut forked_rng = fork_rng(base_rng);
                if let Some(spot) =
                    find_spot_near_building(*b, &mut open_spots_per_road, neighborhoods_roads, map)
                {
                    new_cars += 1;
                    let car = CarID(self.car_id_counter);
                    // TODO since spawning applies during the next step, lots of stuff breaks without
                    // this :(
                    parking_sim.add_parked_car(ParkedCar::new(
                        car,
                        spot,
                        Vehicle::generate_car(car, &mut forked_rng),
                        Some(*b),
                    ));
                    self.car_id_counter += 1;
                } else {
                    // TODO This should be more critical, but neighborhoods can currently contain a
                    // building, but not even its road, so this is inevitable.
                    error!(
                        "No room to seed parked cars. {} total spots, {:?} of {} buildings requested, {} new cars so far. Searched from {}",
                        total_spots,
                        cars_per_building,
                        owner_buildings.len(),
                        new_cars,
                        b
                    );
                }
            }
        }

        info!(
            "Seeded {} of {} parking spots with cars, leaving {} buildings without cars",
            new_cars,
            total_spots,
            owner_buildings.len() - new_cars
        );
    }

    pub fn start_trip_with_car_at_border(
        &mut self,
        at: Tick,
        map: &Map,
        first_lane: LaneID,
        goal: DrivingGoal,
        trips: &mut TripManager,
        base_rng: &mut XorShiftRng,
    ) {
        let car_id = CarID(self.car_id_counter);
        self.car_id_counter += 1;
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        let mut legs = vec![TripLeg::DriveFromBorder(car_id, goal.clone())];
        if let DrivingGoal::ParkNear(b) = goal {
            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
        }
        self.enqueue_command(Command::DriveFromBorder {
            at,
            trip: trips.new_trip(at, ped_id, legs),
            car: car_id,
            vehicle: Vehicle::generate_car(car_id, base_rng),
            start: first_lane,
            goal,
        });
    }

    pub fn start_trip_using_parked_car(
        &mut self,
        at: Tick,
        map: &Map,
        parked: ParkedCar,
        parking_sim: &ParkingSimState,
        start_bldg: BuildingID,
        goal: DrivingGoal,
        trips: &mut TripManager,
    ) {
        // Don't add duplicate commands.
        if let Some(trip) = trips.get_trip_using_car(parked.car) {
            warn!(
                "{} is already a part of {}, ignoring new request",
                parked.car, trip
            );
            return;
        }

        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        let parking_spot = SidewalkSpot::parking_spot(parked.spot, map, parking_sim);

        let mut legs = vec![
            TripLeg::Walk(parking_spot.clone()),
            TripLeg::Drive(parked, goal.clone()),
        ];
        if let DrivingGoal::ParkNear(b) = goal {
            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
        }
        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(at, ped_id, legs),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            parking_spot,
        ));
    }

    pub fn start_trip_using_bike(
        &mut self,
        at: Tick,
        map: &Map,
        start_bldg: BuildingID,
        goal: DrivingGoal,
        trips: &mut TripManager,
        rng: &mut XorShiftRng,
    ) {
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;
        let bike_id = CarID(self.car_id_counter);
        self.car_id_counter += 1;

        let first_spot = {
            let b = map.get_b(start_bldg);
            SidewalkSpot::bike_rack(b.front_path.sidewalk, b.front_path.dist_along_sidewalk, map)
        };

        let mut legs = vec![
            TripLeg::Walk(first_spot.clone()),
            TripLeg::Bike(Vehicle::generate_bike(bike_id, rng), goal.clone()),
        ];
        if let DrivingGoal::ParkNear(b) = goal {
            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
        }
        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(at, ped_id, legs),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            first_spot,
        ));
    }

    pub fn start_trip_with_bike_at_border(
        &mut self,
        at: Tick,
        map: &Map,
        first_lane: LaneID,
        goal: DrivingGoal,
        trips: &mut TripManager,
        base_rng: &mut XorShiftRng,
    ) {
        let bike_id = CarID(self.car_id_counter);
        self.car_id_counter += 1;
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        let vehicle = Vehicle::generate_bike(bike_id, base_rng);

        let mut legs = vec![TripLeg::Bike(vehicle.clone(), goal.clone())];
        if let DrivingGoal::ParkNear(b) = goal {
            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
        }
        self.enqueue_command(Command::DriveFromBorder {
            at,
            trip: trips.new_trip(at, ped_id, legs),
            car: bike_id,
            vehicle,
            start: first_lane,
            goal,
        });
    }

    pub fn start_trip_just_walking(
        &mut self,
        at: Tick,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        trips: &mut TripManager,
    ) {
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(at, ped_id, vec![TripLeg::Walk(goal.clone())]),
            ped_id,
            start,
            goal,
        ));
    }

    // Trip transitions
    pub fn ped_finished_bus_ride(
        &mut self,
        at: Tick,
        ped: PedestrianID,
        stop: BusStopID,
        trips: &mut TripManager,
        map: &Map,
    ) {
        let (trip, walk_to) = trips.ped_finished_bus_ride(ped);
        self.enqueue_command(Command::Walk(
            at.next(),
            trip,
            ped,
            SidewalkSpot::bus_stop(stop, map),
            walk_to,
        ));
    }

    pub fn car_reached_parking_spot(
        &mut self,
        at: Tick,
        p: ParkedCar,
        map: &Map,
        parking_sim: &ParkingSimState,
        trips: &mut TripManager,
    ) {
        let (trip, ped, walk_to) = trips.car_reached_parking_spot(p.car);
        self.enqueue_command(Command::Walk(
            at.next(),
            trip,
            ped,
            SidewalkSpot::parking_spot(p.spot, map, parking_sim),
            walk_to,
        ));
    }

    pub fn bike_reached_end(
        &mut self,
        at: Tick,
        bike: CarID,
        last_lane: LaneID,
        dist: Distance,
        map: &Map,
        trips: &mut TripManager,
    ) {
        let (trip, ped, walk_to) = trips.bike_reached_end(bike);
        self.enqueue_command(Command::Walk(
            at.next(),
            trip,
            ped,
            SidewalkSpot::bike_rack(
                map.get_sidewalk_from_driving_lane(last_lane).unwrap(),
                dist,
                map,
            ),
            walk_to,
        ));
    }

    pub fn ped_ready_to_bike(
        &mut self,
        at: Tick,
        ped: PedestrianID,
        start_sidewalk: LaneID,
        start_dist: Distance,
        trips: &mut TripManager,
    ) {
        let (trip, vehicle, goal) = trips.ped_ready_to_bike(ped);
        self.enqueue_command(Command::Bike {
            at: at.next(),
            trip,
            start_sidewalk,
            start_dist,
            vehicle,
            goal,
        });
    }

    pub fn ped_reached_parking_spot(
        &mut self,
        at: Tick,
        ped: PedestrianID,
        spot: ParkingSpot,
        parking_sim: &ParkingSimState,
        trips: &mut TripManager,
    ) {
        let (trip, goal) = trips.ped_reached_parking_spot(ped);
        self.enqueue_command(Command::Drive(
            at.next(),
            trip,
            parking_sim.get_car_at_spot(spot).unwrap(),
            goal,
        ));
    }

    pub fn is_done(&self) -> bool {
        self.commands.is_empty()
    }

    fn enqueue_command(&mut self, cmd: Command) {
        // TODO Use some kind of priority queue that's serializable
        self.commands.push(cmd);
        // Note the reverse sorting
        self.commands.sort_by(|a, b| b.at().cmp(&a.at()));
    }
}

fn calculate_paths(map: &Map, requests: &Vec<PathRequest>) -> Vec<Option<Path>> {
    use rayon::prelude::*;

    // TODO better timer macro
    let paths: Vec<Option<Path>> = requests
        .par_iter()
        .map(|req| Pathfinder::shortest_distance(map, req.clone()))
        .collect();

    /*debug!(
        "Calculating {} paths took {}s",
        paths.len(),
        elapsed_seconds(timer)
    );*/
    paths
}

// Pick a parking spot for this building. If the building's road has a free spot, use it. If not,
// start BFSing out from the road in a deterministic way until finding a nearby road with an open
// spot.
fn find_spot_near_building(
    b: BuildingID,
    open_spots_per_road: &mut HashMap<RoadID, Vec<ParkingSpot>>,
    neighborhoods_roads: &BTreeSet<RoadID>,
    map: &Map,
) -> Option<ParkingSpot> {
    let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
    let mut visited: HashSet<RoadID> = HashSet::new();
    {
        let start = map.building_to_road(b).id;
        roads_queue.push_back(start);
        visited.insert(start);
    }

    loop {
        if roads_queue.is_empty() {
            warn!(
                "Giving up looking for a free parking spot, searched {} roads of {}: {:?}",
                visited.len(),
                open_spots_per_road.len(),
                visited
            );
        }
        let r = roads_queue.pop_front()?;
        if let Some(spots) = open_spots_per_road.get_mut(&r) {
            // TODO With some probability, skip this available spot and park farther away
            if !spots.is_empty() {
                return spots.pop();
            }
        }

        for next_r in map.get_next_roads(r).into_iter() {
            // Don't floodfill out of the neighborhood
            if !visited.contains(&next_r) && neighborhoods_roads.contains(&next_r) {
                roads_queue.push_back(next_r);
                visited.insert(next_r);
            }
        }
    }
}

// When driving towards some goal building, there may not be a driving lane directly outside the
// building. So BFS out in a deterministic way and find one.
fn find_driving_lane_near_building(b: BuildingID, map: &Map) -> LaneID {
    if let Ok(l) = map.get_driving_lane_from_bldg(b) {
        return l;
    }

    let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
    let mut visited: HashSet<RoadID> = HashSet::new();
    {
        let start = map.building_to_road(b).id;
        roads_queue.push_back(start);
        visited.insert(start);
    }

    loop {
        if roads_queue.is_empty() {
            panic!(
                "Giving up looking for a driving lane near {}, searched {} roads: {:?}",
                b,
                visited.len(),
                visited
            );
        }
        let r = map.get_r(roads_queue.pop_front().unwrap());

        for (lane, lane_type) in r
            .children_forwards
            .iter()
            .chain(r.children_backwards.iter())
        {
            if *lane_type == LaneType::Driving {
                return *lane;
            }
        }

        for next_r in map.get_next_roads(r.id).into_iter() {
            if !visited.contains(&next_r) {
                roads_queue.push_back(next_r);
                visited.insert(next_r);
            }
        }
    }
}

// When biking towards some goal building, there may not be a driving/biking lane directly outside
// the building. So BFS out in a deterministic way and find one.
fn find_biking_goal_near_building(b: BuildingID, map: &Map) -> (LaneID, Distance) {
    // TODO or bike lane
    if let Ok(l) = map.get_driving_lane_from_bldg(b) {
        return (l, map.get_b(b).front_path.dist_along_sidewalk);
    }

    let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
    let mut visited: HashSet<RoadID> = HashSet::new();
    {
        let start = map.building_to_road(b).id;
        roads_queue.push_back(start);
        visited.insert(start);
    }

    loop {
        if roads_queue.is_empty() {
            panic!(
                "Giving up looking for a driving/biking lane near {}, searched {} roads: {:?}",
                b,
                visited.len(),
                visited
            );
        }
        let r = map.get_r(roads_queue.pop_front().unwrap());

        for (lane, lane_type) in r
            .children_forwards
            .iter()
            .chain(r.children_backwards.iter())
        {
            if *lane_type == LaneType::Driving || *lane_type == LaneType::Biking {
                // Just stop in the middle of that road and walk the rest of the way.
                return (*lane, map.get_l(*lane).length() / 2.0);
            }
        }

        for next_r in map.get_next_roads(r.id).into_iter() {
            if !visited.contains(&next_r) {
                roads_queue.push_back(next_r);
                visited.insert(next_r);
            }
        }
    }
}
