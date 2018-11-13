use abstutil::elapsed_seconds;
use dimensioned::si;
use driving::{CreateCar, DrivingGoal, DrivingSimState};
use kinematics::Vehicle;
use map_model::{
    BuildingID, BusRoute, BusStopID, IntersectionID, LaneID, LaneType, Map, Path, Pathfinder,
    RoadID,
};
use parking::ParkingSimState;
use rand::{Rng, XorShiftRng};
use router::Router;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::time::Instant;
use transit::TransitSimState;
use trips::{TripLeg, TripManager};
use walking::{SidewalkSpot, WalkingSimState};
use {
    fork_rng, weighted_sample, AgentID, CarID, Distance, Event, ParkedCar, ParkingSpot,
    PedestrianID, RouteID, Tick, TripID, WeightedUsizeChoice,
};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
enum Command {
    Walk(Tick, TripID, PedestrianID, SidewalkSpot, SidewalkSpot),
    Drive(Tick, TripID, ParkedCar, DrivingGoal),
    DriveFromBorder(Tick, TripID, CarID, Vehicle, LaneID, DrivingGoal),
}

impl Command {
    fn at(&self) -> Tick {
        match self {
            Command::Walk(at, _, _, _, _) => *at,
            Command::Drive(at, _, _, _) => *at,
            Command::DriveFromBorder(at, _, _, _, _, _) => *at,
        }
    }

    fn get_pathfinding_request(
        &self,
        map: &Map,
        parking_sim: &ParkingSimState,
    ) -> (LaneID, Distance, LaneID, Distance) {
        match self {
            Command::Walk(_, _, _, start, goal) => (
                start.sidewalk,
                start.dist_along,
                goal.sidewalk,
                goal.dist_along,
            ),
            Command::Drive(_, _, parked_car, goal) => {
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => find_driving_lane_near_building(*b, map),
                    DrivingGoal::Border(_, l) => *l,
                };
                (
                    map.get_driving_lane_from_parking(parked_car.spot.lane)
                        .unwrap(),
                    parking_sim.dist_along_for_car(parked_car.spot, &parked_car.vehicle),
                    goal_lane,
                    map.get_l(goal_lane).length(),
                )
            }
            Command::DriveFromBorder(_, _, _, _, start, goal) => {
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => find_driving_lane_near_building(*b, map),
                    DrivingGoal::Border(_, l) => *l,
                };
                (
                    *start,
                    0.0 * si::M,
                    goal_lane,
                    map.get_l(goal_lane).length(),
                )
            }
        }
    }

    fn retry_next_tick(&self) -> Command {
        match self {
            Command::Walk(at, trip, ped, start, goal) => {
                Command::Walk(at.next(), *trip, *ped, start.clone(), goal.clone())
            }
            Command::Drive(at, trip, parked_car, goal) => {
                Command::Drive(at.next(), *trip, parked_car.clone(), goal.clone())
            }
            Command::DriveFromBorder(at, trip, car, vehicle, start, goal) => {
                Command::DriveFromBorder(
                    at.next(),
                    *trip,
                    *car,
                    vehicle.clone(),
                    *start,
                    goal.clone(),
                )
            }
        }
    }
}

// This owns car/ped IDs.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
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
        events: &mut Vec<Event>,
        now: Tick,
        map: &Map,
        parking_sim: &mut ParkingSimState,
        walking_sim: &mut WalkingSimState,
        driving_sim: &mut DrivingSimState,
        trips: &mut TripManager,
    ) {
        let mut commands: Vec<Command> = Vec::new();
        let mut requested_paths: Vec<(LaneID, Distance, LaneID, Distance)> = Vec::new();
        loop {
            if self
                .commands
                .last()
                .and_then(|cmd| Some(now == cmd.at()))
                .unwrap_or(false)
            {
                let cmd = self.commands.pop().unwrap();
                requested_paths.push(cmd.get_pathfinding_request(map, parking_sim));
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
                    Command::Drive(_, trip, ref parked_car, ref goal) => {
                        let car = parked_car.car;

                        // TODO this looks like it jumps when the parking and driving lanes are different lengths
                        // due to diagonals
                        let dist_along =
                            parking_sim.dist_along_for_car(parked_car.spot, &parked_car.vehicle);
                        let start = path.current_step().as_traversable().as_lane();
                        if driving_sim.start_car_on_lane(
                            events,
                            now,
                            map,
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
                        ) {
                            trips.agent_starting_trip_leg(AgentID::Car(car), trip);
                            parking_sim.remove_parked_car(parked_car.clone());
                            spawned_agents += 1;
                        } else {
                            self.enqueue_command(cmd.retry_next_tick());
                        }
                    }
                    Command::DriveFromBorder(_, trip, car, ref vehicle, start, ref goal) => {
                        if driving_sim.start_car_on_lane(
                            events,
                            now,
                            map,
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
                        ) {
                            trips.agent_starting_trip_leg(AgentID::Car(car), trip);
                            spawned_agents += 1;
                        } else {
                            self.enqueue_command(cmd.retry_next_tick());
                        }
                    }
                    Command::Walk(_, trip, ped, start, goal) => {
                        trips.agent_starting_trip_leg(AgentID::Pedestrian(ped), trip);
                        walking_sim.seed_pedestrian(events, ped, trip, start, goal, path);
                        spawned_agents += 1;
                    }
                };
            } else {
                error!(
                    "Couldn't find path from {} to {} for {:?}",
                    req.0, req.2, cmd
                );
            }
        }
        debug!(
            "Spawned {} agents of requested {}",
            spawned_agents,
            requested_paths.len()
        );
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
            for _ in 0..weighted_sample(&cars_per_building, base_rng) {
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
                        Vehicle::generate_typical_car(car, base_rng),
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
                    // Kind of a hack, but don't let the RNG get out of sync because of this. Not
                    // happy about passing in a dummy CarID.
                    Vehicle::generate_typical_car(CarID(0), base_rng);
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

    pub fn seed_specific_parked_cars(
        &mut self,
        lane: LaneID,
        owner: BuildingID,
        spot_indices: Vec<usize>,
        parking_sim: &mut ParkingSimState,
        rng: &mut XorShiftRng,
    ) -> Vec<CarID> {
        let spots = parking_sim.get_all_spots(lane);
        spot_indices
            .into_iter()
            .map(|idx| {
                let car = CarID(self.car_id_counter);
                parking_sim.add_parked_car(ParkedCar::new(
                    car,
                    spots[idx],
                    Vehicle::generate_typical_car(car, rng),
                    Some(owner),
                ));
                self.car_id_counter += 1;
                car
            }).collect()
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
        self.enqueue_command(Command::DriveFromBorder(
            at,
            trips.new_trip(at, ped_id, legs),
            car_id,
            Vehicle::generate_typical_car(car_id, base_rng),
            first_lane,
            goal,
        ));
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

    pub fn start_trip_using_bus(
        &mut self,
        at: Tick,
        map: &Map,
        start_bldg: BuildingID,
        goal_bldg: BuildingID,
        stop1: BusStopID,
        stop2: BusStopID,
        route: RouteID,
        trips: &mut TripManager,
    ) -> PedestrianID {
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(
                at,
                ped_id,
                vec![
                    TripLeg::Walk(SidewalkSpot::bus_stop(stop1, map)),
                    TripLeg::RideBus(route, stop2),
                    TripLeg::Walk(SidewalkSpot::building(goal_bldg, map)),
                ],
            ),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            SidewalkSpot::bus_stop(stop1, map),
        ));
        ped_id
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

fn calculate_paths(
    requested_paths: &Vec<(LaneID, Distance, LaneID, Distance)>,
    map: &Map,
) -> Vec<Option<Path>> {
    use rayon::prelude::*;

    debug!("Calculating {} paths", requested_paths.len());
    // TODO better timer macro
    let timer = Instant::now();
    let paths: Vec<Option<Path>> = requested_paths
        .par_iter()
        // TODO No bikes yet, so never use the bike lanes
        // TODO I don't think buses ever use this, so also hardcode false. requested_paths should
        // be a struct of the required input to shortest_distance, probably.
        .map(|(start, start_dist, goal, goal_dist)| {
            Pathfinder::shortest_distance(map, *start, *start_dist, *goal, *goal_dist, false, false)
        }).collect();

    debug!(
        "Calculating {} paths took {}s",
        paths.len(),
        elapsed_seconds(timer)
    );
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
