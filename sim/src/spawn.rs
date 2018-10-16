use abstutil::elapsed_seconds;
use driving::DrivingSimState;
use kinematics::Vehicle;
use map_model::{BuildingID, BusRoute, BusStopID, LaneID, Map, Pathfinder};
use parking::ParkingSimState;
use rand::Rng;
use router::Router;
use std::collections::VecDeque;
use std::time::Instant;
use transit::TransitSimState;
use trips::{TripLeg, TripManager};
use walking::{SidewalkSpot, WalkingSimState};
use {
    fork_rng, AgentID, CarID, Event, ParkedCar, ParkingSpot, PedestrianID, RouteID, Tick, TripID,
};

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
        let mut requested_paths: Vec<(LaneID, LaneID)> = Vec::new();
        loop {
            if self
                .commands
                .last()
                .and_then(|cmd| Some(now == cmd.at()))
                .unwrap_or(false)
            {
                let cmd = self.commands.pop().unwrap();
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
            if let Some(mut path) = maybe_path {
                match cmd {
                    Command::Drive(_, trip, ref parked_car, goal_bldg) => {
                        let car = parked_car.car;

                        // TODO this looks like it jumps when the parking and driving lanes are different lengths
                        // due to diagonals
                        let dist_along =
                            parking_sim.dist_along_for_car(parked_car.spot, &parked_car.vehicle);
                        let start = path.pop_front().unwrap();
                        if driving_sim.start_car_on_lane(
                            events,
                            now,
                            car,
                            Some(trip),
                            Some(parked_car.clone()),
                            parked_car.vehicle.clone(),
                            dist_along,
                            start,
                            Router::make_router_to_park(path, goal_bldg),
                            map,
                        ) {
                            trips.agent_starting_trip_leg(AgentID::Car(car), trip);
                            parking_sim.remove_parked_car(parked_car.clone());
                            spawned_agents += 1;
                        } else {
                            self.enqueue_command(cmd.retry_next_tick());
                        }
                    }
                    Command::Walk(_, trip, ped, spot1, spot2) => {
                        trips.agent_starting_trip_leg(AgentID::Pedestrian(ped), trip);
                        walking_sim.seed_pedestrian(events, ped, trip, spot1, spot2, map, path);
                        spawned_agents += 1;
                    }
                };
            } else {
                debug!(
                    "Couldn't find path from {} to {} for {:?}",
                    req.0, req.1, cmd
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
    pub fn seed_bus_route<R: Rng + ?Sized>(
        &mut self,
        events: &mut Vec<Event>,
        route: &BusRoute,
        rng: &mut R,
        map: &Map,
        driving_sim: &mut DrivingSimState,
        transit_sim: &mut TransitSimState,
        now: Tick,
    ) -> Vec<CarID> {
        let route_id = transit_sim.create_empty_route(route, map);
        let mut results: Vec<CarID> = Vec::new();
        // Try to spawn a bus at each stop
        for (next_stop_idx, start_dist_along, mut path) in
            transit_sim.get_route_starts(route_id, map).into_iter()
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
                None,
                vehicle,
                start_dist_along,
                start,
                Router::make_router_for_bus(path),
                map,
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
    pub fn seed_parked_cars<R: Rng + ?Sized>(
        &mut self,
        percent_capacity_to_fill: f64,
        in_lanes: Vec<LaneID>,
        parking_sim: &mut ParkingSimState,
        base_rng: &mut R,
    ) {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);

        let mut total_capacity = 0;
        let mut new_cars = 0;
        // Fork a new RNG for each candidate lane. This keeps things more deterministic, invariant
        // of lane edits.
        for l in in_lanes.into_iter() {
            let mut rng = fork_rng(base_rng);

            for spot in parking_sim.get_free_spots(l) {
                total_capacity += 1;
                if rng.gen_bool(percent_capacity_to_fill) {
                    new_cars += 1;
                    let car = CarID(self.car_id_counter);
                    // TODO since spawning applies during the next step, lots of stuff breaks without
                    // this :(
                    parking_sim.add_parked_car(ParkedCar::new(
                        car,
                        spot,
                        Vehicle::generate_typical_car(car, &mut rng),
                    ));
                    self.car_id_counter += 1;
                }
            }
        }
        info!(
            "Seeded {} of {} parking spots with cars",
            new_cars, total_capacity
        );
    }

    pub fn seed_specific_parked_cars<R: Rng + ?Sized>(
        &mut self,
        lane: LaneID,
        spot_indices: Vec<usize>,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
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
                ));
                self.car_id_counter += 1;
                car
            }).collect()
    }

    pub fn start_trip_using_parked_car(
        &mut self,
        at: Tick,
        map: &Map,
        parked: ParkedCar,
        parking_sim: &ParkingSimState,
        start_bldg: BuildingID,
        goal_bldg: BuildingID,
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
        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(
                at,
                map,
                ped_id,
                start_bldg,
                goal_bldg,
                vec![
                    TripLeg::Walk(parking_spot.clone()),
                    TripLeg::Drive(parked, goal_bldg),
                    TripLeg::Walk(SidewalkSpot::building(goal_bldg, map)),
                ],
            ),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            parking_spot,
        ));
    }

    pub fn start_trip_just_walking(
        &mut self,
        at: Tick,
        map: &Map,
        start_bldg: BuildingID,
        goal_bldg: BuildingID,
        trips: &mut TripManager,
    ) {
        let ped_id = PedestrianID(self.ped_id_counter);
        self.ped_id_counter += 1;

        self.enqueue_command(Command::Walk(
            at,
            trips.new_trip(
                at,
                map,
                ped_id,
                start_bldg,
                goal_bldg,
                vec![TripLeg::Walk(SidewalkSpot::building(goal_bldg, map))],
            ),
            ped_id,
            SidewalkSpot::building(start_bldg, map),
            SidewalkSpot::building(goal_bldg, map),
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
                map,
                ped_id,
                start_bldg,
                goal_bldg,
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
        let (trip, goal_bldg) = trips.ped_reached_parking_spot(ped);
        self.enqueue_command(Command::Drive(
            at.next(),
            trip,
            parking_sim.get_car_at_spot(spot).unwrap(),
            goal_bldg,
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
    requested_paths: &Vec<(LaneID, LaneID)>,
    map: &Map,
) -> Vec<Option<VecDeque<LaneID>>> {
    use rayon::prelude::*;

    debug!("Calculating {} paths", requested_paths.len());
    // TODO better timer macro
    let timer = Instant::now();
    let paths: Vec<Option<VecDeque<LaneID>>> = requested_paths
        .par_iter()
        // TODO No bikes yet, so never use the bike lanes
        .map(|(start, goal)| Pathfinder::shortest_distance(map, *start, *goal, false))
        .collect();

    debug!(
        "Calculating {} paths took {}s",
        paths.len(),
        elapsed_seconds(timer)
    );
    paths
}
