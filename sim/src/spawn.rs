use crate::driving::{CreateCar, DrivingGoal, DrivingSimState};
use crate::intersections::IntersectionSimState;
use crate::kinematics::{Vehicle, BUS_LENGTH, MAX_BIKE_LENGTH};
use crate::parking::ParkingSimState;
use crate::router::Router;
use crate::scheduler;
use crate::transit::TransitSimState;
use crate::trips::{TripLeg, TripManager};
use crate::walking::{CreatePedestrian, SidewalkSpot};
use crate::{
    AgentID, CarID, Event, ParkedCar, ParkingSpot, PedestrianID, Tick, TripID, VehicleType,
};
use abstutil::{fork_rng, Timer, WeightedUsizeChoice};
use geom::Distance;
use map_model::{
    BuildingID, BusRoute, BusRouteID, BusStopID, LaneID, LaneType, Map, Path, PathRequest,
    Pathfinder, Position, RoadID,
};
use rand::seq::SliceRandom;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
enum Command {
    Walk(Tick, TripID, PedestrianID, SidewalkSpot, SidewalkSpot),
    Drive(Tick, TripID, ParkedCar, DrivingGoal),
    // As in, a car appears suddenly
    DriveSpontaneously {
        at: Tick,
        trip: TripID,
        car: CarID,
        vehicle: Vehicle,
        start: Position,
        goal: DrivingGoal,
    },
    Bike {
        at: Tick,
        trip: TripID,
        start_sidewalk: Position,
        vehicle: Vehicle,
        goal: DrivingGoal,
    },
}

impl Command {
    fn at(&self) -> Tick {
        match self {
            Command::Walk(at, _, _, _, _) => *at,
            Command::Drive(at, _, _, _) => *at,
            Command::DriveSpontaneously { at, .. } => *at,
            Command::Bike { at, .. } => *at,
        }
    }

    fn get_pathfinding_request(&self, map: &Map, parking_sim: &ParkingSimState) -> PathRequest {
        match self {
            Command::Walk(_, _, _, start, goal) => PathRequest {
                start: start.sidewalk_pos,
                end: goal.sidewalk_pos,
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            Command::Drive(_, _, parked_car, goal) => {
                let start_lane = map
                    .find_closest_lane(parked_car.spot.lane, vec![LaneType::Driving])
                    .unwrap();
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => map.find_driving_lane_near_building(*b),
                    DrivingGoal::Border(_, l) => *l,
                };

                PathRequest {
                    start: parking_sim.spot_to_driving_pos(
                        parked_car.spot,
                        &parked_car.vehicle,
                        start_lane,
                        map,
                    ),
                    end: Position::new(goal_lane, map.get_l(goal_lane).length()),
                    can_use_bike_lanes: false,
                    can_use_bus_lanes: false,
                }
            }
            Command::Bike {
                start_sidewalk,
                goal,
                ..
            } => {
                let start_lane = map
                    .find_closest_lane(
                        start_sidewalk.lane(),
                        vec![LaneType::Driving, LaneType::Biking],
                    )
                    .unwrap();
                let start = start_sidewalk.equiv_pos(start_lane, map);
                let end = match goal {
                    DrivingGoal::ParkNear(b) => find_biking_goal_near_building(*b, map),
                    DrivingGoal::Border(_, l) => Position::new(*l, map.get_l(*l).length()),
                };
                PathRequest {
                    start,
                    end,
                    can_use_bus_lanes: false,
                    can_use_bike_lanes: true,
                }
            }
            Command::DriveSpontaneously {
                start,
                goal,
                vehicle,
                ..
            } => {
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => map.find_driving_lane_near_building(*b),
                    DrivingGoal::Border(_, l) => *l,
                };
                PathRequest {
                    start: *start,
                    end: Position::new(goal_lane, map.get_l(goal_lane).length()),
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

                        let start_lane = path.current_step().as_traversable().as_lane();
                        let start = parking_sim.spot_to_driving_pos(
                            parked_car.spot,
                            &parked_car.vehicle,
                            start_lane,
                            map,
                        );

                        scheduler.enqueue_command(scheduler::Command::SpawnCar(
                            now,
                            CreateCar {
                                car,
                                trip,
                                owner: parked_car.owner,
                                maybe_parked_car: Some(parked_car.clone()),
                                vehicle: parked_car.vehicle.clone(),
                                start,
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
                    Command::DriveSpontaneously {
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
                                trip,
                                // TODO need a way to specify this in the scenario
                                owner: None,
                                maybe_parked_car: None,
                                vehicle: vehicle.clone(),
                                start,
                                router: match goal {
                                    DrivingGoal::ParkNear(b) => {
                                        if vehicle.vehicle_type == VehicleType::Bike {
                                            Router::make_bike_router(path, req.end.dist_along())
                                        } else {
                                            Router::make_router_to_park(path, *b)
                                        }
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
                                trip,
                                owner: None,
                                maybe_parked_car: None,
                                vehicle: vehicle.clone(),
                                start: req.start,
                                router: match goal {
                                    DrivingGoal::ParkNear(_) => {
                                        Router::make_bike_router(path, req.end.dist_along())
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
                    "Couldn't find path at {} from {} to {} for {:?}",
                    now, req.start, req.end, cmd
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
        intersections: &IntersectionSimState,
        trips: &mut TripManager,
        now: Tick,
    ) -> Vec<CarID> {
        transit_sim.create_empty_route(route, map);
        let mut results: Vec<CarID> = Vec::new();
        // Try to spawn a bus at each stop
        for (next_stop_idx, start_dist_along, path) in
            transit_sim.get_route_starts(route.id, map).into_iter()
        {
            let start = Position::new(
                path.current_step().as_traversable().as_lane(),
                start_dist_along,
            );

            if start_dist_along < BUS_LENGTH {
                warn!(
                    "Stop at {:?} is too short to spawn a bus there; giving up on one bus for {}",
                    start, route.id
                );
                continue;
            }

            let id = CarID(self.car_id_counter, VehicleType::Bus);
            self.car_id_counter += 1;
            let vehicle = Vehicle::generate_bus(id, rng);

            // TODO Aww, we create an orphan trip if the bus can't spawn.
            let trip = trips.new_trip(now, None, vec![TripLeg::ServeBusRoute(id, route.id)]);
            if driving_sim.start_car_on_lane(
                events,
                now,
                map,
                CreateCar {
                    car: id,
                    trip,
                    owner: None,
                    maybe_parked_car: None,
                    vehicle,
                    start,
                    router: Router::make_router_for_bus(path),
                },
                intersections,
            ) {
                trips.agent_starting_trip_leg(AgentID::Car(id), trip);
                transit_sim.bus_created(id, route.id, next_stop_idx);
                info!("Spawned bus {} for route {} ({})", id, route.name, route.id);
                results.push(id);
            } else {
                warn!(
                    "No room for a bus headed towards stop {} of {} ({}), giving up",
                    next_stop_idx, route.name, route.id
                );
            }
        }
        results
    }

    // This happens immediately; it isn't scheduled.
    // TODO This is for tests; rename or move it?
    // TODO duplication of code, weird responsibilities here...
    pub fn seed_specific_parked_cars(
        &mut self,
        lane: LaneID,
        owner_building: BuildingID,
        spots: Vec<usize>,
        parking_sim: &mut ParkingSimState,
        base_rng: &mut XorShiftRng,
    ) -> Vec<CarID> {
        let mut results: Vec<CarID> = Vec::new();
        for idx in spots.into_iter() {
            let car = CarID(self.car_id_counter, VehicleType::Car);
            parking_sim.add_parked_car(ParkedCar::new(
                car,
                ParkingSpot::new(lane, idx),
                Vehicle::generate_car(car, base_rng),
                Some(owner_building),
            ));
            self.car_id_counter += 1;
            results.push(car);
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
        timer: &mut Timer,
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
            spots.shuffle(&mut fork_rng(base_rng));
            open_spots_per_road.insert(r.id, spots);
        }

        let mut new_cars = 0;
        timer.start_iter("seed parked cars for buildings", owner_buildings.len());
        for b in owner_buildings {
            timer.next();
            for _ in 0..cars_per_building.sample(base_rng) {
                let mut forked_rng = fork_rng(base_rng);
                if let Some(spot) =
                    find_spot_near_building(*b, &mut open_spots_per_road, neighborhoods_roads, map)
                {
                    new_cars += 1;
                    let car = CarID(self.car_id_counter, VehicleType::Car);
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

    // Usually first_lane is a border, but could also be anywhere else for interactive debugging.
    pub fn start_trip_with_car_appearing(
        &mut self,
        at: Tick,
        map: &Map,
        mut from: Position,
        goal: DrivingGoal,
        trips: &mut TripManager,
        base_rng: &mut XorShiftRng,
    ) -> CarID {
        let car_id = CarID(self.car_id_counter, VehicleType::Car);
        self.car_id_counter += 1;
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        // TODO Should rephrase TripLeg and Command to not assume borders, but... things seem
        // to work fine.
        let mut legs = vec![TripLeg::DriveFromBorder(car_id, goal.clone())];
        if let DrivingGoal::ParkNear(b) = goal {
            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
        }
        let vehicle = Vehicle::generate_car(car_id, base_rng);
        if map.get_l(from.lane()).length() < vehicle.length {
            panic!(
                "Can't spawn a car at {}; it's only {}, but vehicle length is {}",
                from.lane(),
                map.get_l(from.lane()).length(),
                vehicle.length
            );
        }
        // Fix up the position if the start was requested.
        if from.dist_along() == Distance::ZERO {
            from = Position::new(from.lane(), vehicle.length);
        }
        self.enqueue_command(Command::DriveSpontaneously {
            at,
            trip: trips.new_trip(at, Some(ped_id), legs),
            car: car_id,
            vehicle,
            start: from,
            goal,
        });
        car_id
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

        assert_eq!(parked.owner, Some(start_bldg));

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
            trips.new_trip(at, Some(ped_id), legs),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            parking_spot,
        ));
    }

    // This might fail!
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
        let bike_id = CarID(self.car_id_counter, VehicleType::Bike);
        self.car_id_counter += 1;

        let first_spot = {
            let b = map.get_b(start_bldg);
            let pos = b.front_path.sidewalk;
            if pos.dist_along() < MAX_BIKE_LENGTH {
                let lane_len = map.get_l(pos.lane()).length();
                if lane_len < MAX_BIKE_LENGTH {
                    // Just give up
                    return;
                }
                SidewalkSpot::bike_rack(Position::new(pos.lane(), MAX_BIKE_LENGTH), map)
            } else {
                SidewalkSpot::bike_rack(pos, map)
            }
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
            trips.new_trip(at, Some(ped_id), legs),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            first_spot,
        ));
    }

    pub fn start_trip_using_bus(
        &mut self,
        at: Tick,
        map: &Map,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        route: BusRouteID,
        stop1: BusStopID,
        stop2: BusStopID,
        trips: &mut TripManager,
    ) -> PedestrianID {
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        let first_stop = SidewalkSpot::bus_stop(stop1, map);
        let legs = vec![
            TripLeg::Walk(first_stop.clone()),
            TripLeg::RideBus(route, stop2),
            TripLeg::Walk(goal),
        ];
        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(at, Some(ped_id), legs),
            ped_id,
            start,
            first_stop,
        ));
        ped_id
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
        let bike_id = CarID(self.car_id_counter, VehicleType::Bike);
        self.car_id_counter += 1;
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        let vehicle = Vehicle::generate_bike(bike_id, base_rng);

        let mut legs = vec![TripLeg::Bike(vehicle.clone(), goal.clone())];
        if let DrivingGoal::ParkNear(b) = goal {
            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
        }
        assert!(map.get_l(first_lane).length() > vehicle.length);
        self.enqueue_command(Command::DriveSpontaneously {
            at,
            trip: trips.new_trip(at, Some(ped_id), legs),
            car: bike_id,
            start: Position::new(first_lane, vehicle.length),
            vehicle,
            goal,
        });
    }

    pub fn start_trip_just_walking(
        &mut self,
        at: Tick,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        trips: &mut TripManager,
    ) -> PedestrianID {
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(at, Some(ped_id), vec![TripLeg::Walk(goal.clone())]),
            ped_id,
            start,
            goal,
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

    pub fn bike_reached_end(
        &mut self,
        at: Tick,
        bike: CarID,
        last_driving_pos: Position,
        map: &Map,
        trips: &mut TripManager,
    ) {
        let (trip, ped, walk_to) = trips.bike_reached_end(bike);
        self.enqueue_command(Command::Walk(
            at.next(),
            trip,
            ped,
            SidewalkSpot::bike_rack(
                last_driving_pos.equiv_pos(
                    map.find_closest_lane(last_driving_pos.lane(), vec![LaneType::Sidewalk])
                        .unwrap(),
                    map,
                ),
                map,
            ),
            walk_to,
        ));
    }

    pub fn ped_ready_to_bike(
        &mut self,
        at: Tick,
        ped: PedestrianID,
        sidewalk_pos: Position,
        trips: &mut TripManager,
    ) {
        let (trip, vehicle, goal) = trips.ped_ready_to_bike(ped);
        self.enqueue_command(Command::Bike {
            at: at.next(),
            trip,
            start_sidewalk: sidewalk_pos,
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

// When biking towards some goal building, there may not be a driving/biking lane directly outside
// the building. So BFS out in a deterministic way and find one.
fn find_biking_goal_near_building(b: BuildingID, map: &Map) -> Position {
    if let Ok(l) = map.find_closest_lane_to_bldg(b, vec![LaneType::Driving, LaneType::Biking]) {
        return map.get_b(b).front_path.sidewalk.equiv_pos(l, map);
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
                return Position::new(*lane, map.get_l(*lane).length() / 2.0);
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
