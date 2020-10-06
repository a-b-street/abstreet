use std::collections::{BTreeSet, HashSet};
use std::panic;

use instant::Instant;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};

use abstutil::{prettyprint_usize, serialized_size_bytes, CmdArgs, Parallelism, Timer};
use geom::{Distance, Duration, Speed, Time};
use map_model::{
    BuildingID, BusRoute, LaneID, Map, ParkingLotID, Path, PathConstraints, PathRequest, Position,
    Traversable,
};

pub use self::queries::AgentProperties;
use crate::{
    AgentID, AlertLocation, Analytics, CapSimState, CarID, Command, CreateCar, DrawCarInput,
    DrawPedCrowdInput, DrawPedestrianInput, DrivingSimState, Event, GetDrawAgents,
    IntersectionSimState, OrigPersonID, PandemicModel, ParkedCar, ParkingSim, ParkingSimState,
    ParkingSpot, PedestrianID, Person, PersonID, Router, Scheduler, SidewalkPOI, SidewalkSpot,
    TransitSimState, TripID, TripManager, TripPhaseType, TripSpawner, UnzoomedAgent, Vehicle,
    VehicleSpec, VehicleType, WalkingSimState, BUS_LENGTH, LIGHT_RAIL_LENGTH, MIN_CAR_LENGTH,
    SPAWN_DIST,
};

mod queries;

// TODO Do something else.
const BLIND_RETRY_TO_SPAWN: Duration = Duration::const_seconds(5.0);

#[derive(Serialize, Deserialize, Clone)]
pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    transit: TransitSimState,
    cap: CapSimState,
    trips: TripManager,
    #[serde(skip_serializing, skip_deserializing)]
    pandemic: Option<PandemicModel>,
    scheduler: Scheduler,
    time: Time,

    // TODO Reconsider these
    pub(crate) map_name: String,
    pub(crate) edits_name: String,
    // Some tests deliberately set different scenario names for comparisons.
    // TODO Maybe get rid of this, now that savestates aren't used
    run_name: String,
    step_count: usize,

    // Don't serialize, to reduce prebaked savestate size. Analytics are saved once covering the
    // full day and can be trimmed to any time.
    #[serde(skip_serializing, skip_deserializing)]
    analytics: Analytics,

    #[serde(skip_serializing, skip_deserializing)]
    alerts: AlertHandler,
}

pub struct Ctx<'a> {
    pub parking: &'a mut ParkingSimState,
    pub intersections: &'a mut IntersectionSimState,
    pub cap: &'a mut CapSimState,
    pub scheduler: &'a mut Scheduler,
    pub map: &'a Map,
}

#[derive(Clone)]
pub struct SimOptions {
    pub run_name: String,
    pub use_freeform_policy_everywhere: bool,
    pub dont_block_the_box: bool,
    pub recalc_lanechanging: bool,
    pub break_turn_conflict_cycles: bool,
    pub handle_uber_turns: bool,
    pub enable_pandemic_model: Option<XorShiftRng>,
    pub alerts: AlertHandler,
    pub pathfinding_upfront: bool,
    pub infinite_parking: bool,
}

impl std::default::Default for SimOptions {
    fn default() -> SimOptions {
        SimOptions::new("tmp")
    }
}

impl SimOptions {
    pub fn from_args(args: &mut CmdArgs, rng_seed: u8) -> SimOptions {
        SimOptions {
            run_name: args
                .optional("--run_name")
                .unwrap_or_else(|| "unnamed".to_string()),
            use_freeform_policy_everywhere: args.enabled("--freeform_policy"),
            dont_block_the_box: !args.enabled("--disable_block_the_box"),
            recalc_lanechanging: !args.enabled("--disable_recalc_lc"),
            break_turn_conflict_cycles: !args.enabled("--disable_break_turn_conflict_cycles"),
            handle_uber_turns: !args.enabled("--disable_handle_uber_turns"),
            enable_pandemic_model: if args.enabled("--pandemic") {
                Some(XorShiftRng::from_seed([rng_seed; 16]))
            } else {
                None
            },
            alerts: args
                .optional("--alerts")
                .map(|x| match x.as_ref() {
                    "print" => AlertHandler::Print,
                    "block" => AlertHandler::Block,
                    "silence" => AlertHandler::Silence,
                    _ => panic!("Bad --alerts={}. Must be print|block|silence", x),
                })
                .unwrap_or(AlertHandler::Print),
            pathfinding_upfront: args.enabled("--pathfinding_upfront"),
            infinite_parking: args.enabled("--infinite_parking"),
        }
    }
}

#[derive(Clone)]
pub enum AlertHandler {
    // Just print the alert to STDOUT
    Print,
    // Print the alert to STDOUT and don't proceed until the UI calls clear_alerts()
    Block,
    // Don't do anything
    Silence,
}

impl std::default::Default for AlertHandler {
    fn default() -> AlertHandler {
        AlertHandler::Print
    }
}

impl SimOptions {
    pub fn new(run_name: &str) -> SimOptions {
        SimOptions {
            run_name: run_name.to_string(),
            use_freeform_policy_everywhere: false,
            dont_block_the_box: true,
            recalc_lanechanging: true,
            break_turn_conflict_cycles: true,
            handle_uber_turns: true,
            enable_pandemic_model: None,
            alerts: AlertHandler::Print,
            pathfinding_upfront: false,
            infinite_parking: false,
        }
    }
}

// Setup
impl Sim {
    pub fn new(map: &Map, opts: SimOptions, timer: &mut Timer) -> Sim {
        let mut scheduler = Scheduler::new();
        Sim {
            driving: DrivingSimState::new(map, opts.recalc_lanechanging, opts.handle_uber_turns),
            parking: ParkingSimState::new(map, opts.infinite_parking, timer),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(
                map,
                &mut scheduler,
                opts.use_freeform_policy_everywhere,
                opts.dont_block_the_box,
                opts.break_turn_conflict_cycles,
                opts.handle_uber_turns,
            ),
            transit: TransitSimState::new(map),
            cap: CapSimState::new(map),
            trips: TripManager::new(opts.pathfinding_upfront),
            pandemic: if let Some(rng) = opts.enable_pandemic_model {
                Some(PandemicModel::new(rng))
            } else {
                None
            },
            scheduler,
            time: Time::START_OF_DAY,

            map_name: map.get_name().to_string(),
            edits_name: map.get_edits().edits_name.clone(),
            run_name: opts.run_name,
            step_count: 0,
            alerts: opts.alerts,

            analytics: Analytics::new(),
        }
    }

    pub fn make_spawner(&self) -> TripSpawner {
        TripSpawner::new()
    }
    pub fn flush_spawner(&mut self, spawner: TripSpawner, map: &Map, timer: &mut Timer) {
        spawner.finalize(map, &mut self.trips, &mut self.scheduler, timer);

        if let Some(ref mut m) = self.pandemic {
            m.initialize(self.trips.get_all_people(), &mut self.scheduler);
        }

        self.dispatch_events(Vec::new(), map);
    }

    pub fn get_free_onstreet_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_onstreet_spots(l)
    }

    pub fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot> {
        self.parking.get_free_offstreet_spots(b)
    }

    pub fn get_free_lot_spots(&self, pl: ParkingLotID) -> Vec<ParkingSpot> {
        self.parking.get_free_lot_spots(pl)
    }

    // (Filled, available)
    pub fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        self.parking.get_all_parking_spots()
    }

    pub fn bldg_to_parked_cars(&self, b: BuildingID) -> Vec<CarID> {
        self.parking.bldg_to_parked_cars(b)
    }

    // Also returns the start distance of the building. TODO Do that in the Path properly.
    pub fn walking_path_to_nearest_parking_spot(
        &self,
        map: &Map,
        b: BuildingID,
    ) -> Option<(Path, Distance)> {
        let vehicle = Vehicle {
            id: CarID(0, VehicleType::Car),
            owner: None,
            vehicle_type: VehicleType::Car,
            length: MIN_CAR_LENGTH,
            max_speed: None,
        };
        let driving_lane = map.find_driving_lane_near_building(b);

        // Anything on the current lane? TODO Should find the closest one to the sidewalk, but
        // need a new method in ParkingSimState to make that easy.
        // TODO Refactor the logic in router
        let spot = if let Some((spot, _)) = self
            .parking
            .get_all_free_spots(Position::start(driving_lane), &vehicle, b, map)
            .get(0)
        {
            spot.clone()
        } else {
            let (_, spot, _) =
                self.parking
                    .path_to_free_parking_spot(driving_lane, &vehicle, b, map)?;
            spot
        };

        let start = SidewalkSpot::building(b, map).sidewalk_pos;
        let end = SidewalkSpot::parking_spot(spot, map, &self.parking).sidewalk_pos;
        let path = map.pathfind(PathRequest {
            start,
            end,
            constraints: PathConstraints::Pedestrian,
        })?;
        Some((path, start.dist_along()))
    }

    // TODO Should these two be in TripSpawner?
    pub(crate) fn new_person(
        &mut self,
        p: PersonID,
        orig_id: Option<OrigPersonID>,
        ped_speed: Speed,
        vehicle_specs: Vec<VehicleSpec>,
    ) {
        self.trips.new_person(p, orig_id, ped_speed, vehicle_specs);
    }
    pub fn random_person(&mut self, ped_speed: Speed, vehicle_specs: Vec<VehicleSpec>) -> &Person {
        self.trips.random_person(ped_speed, vehicle_specs)
    }
    pub(crate) fn seed_parked_car(&mut self, vehicle: Vehicle, spot: ParkingSpot) {
        self.parking.reserve_spot(spot);
        self.parking.add_parked_car(ParkedCar {
            vehicle,
            spot,
            parked_since: self.time,
        });
    }

    pub(crate) fn seed_bus_route(&mut self, route: &BusRoute) {
        for t in &route.spawn_times {
            self.scheduler.push(*t, Command::StartBus(route.id, *t));
        }
    }

    fn start_bus(&mut self, route: &BusRoute, map: &Map) {
        // Spawn one bus for the first leg.
        let (req, path) = self.transit.create_empty_route(route, map);

        // For now, no desire for randomness. Caller can pass in list of specs if that ever
        // changes.
        let (vehicle_type, length) = match route.route_type {
            PathConstraints::Bus => (VehicleType::Bus, BUS_LENGTH),
            PathConstraints::Train => (VehicleType::Train, LIGHT_RAIL_LENGTH),
            _ => unreachable!(),
        };
        let vehicle = VehicleSpec {
            vehicle_type,
            length,
            max_speed: None,
        }
        .make(CarID(self.trips.new_car_id(), vehicle_type), None);
        let start_lane = map.get_l(path.current_step().as_lane());
        let start_dist = if map.get_i(start_lane.src_i).is_incoming_border() {
            SPAWN_DIST
        } else {
            assert!(start_lane.length() > vehicle.length);
            vehicle.length
        };

        self.scheduler.push(
            self.time,
            Command::SpawnCar(
                CreateCar {
                    start_dist,
                    router: Router::follow_bus_route(
                        vehicle.id,
                        path.clone(),
                        req.end.dist_along(),
                    ),
                    vehicle,
                    req,
                    maybe_parked_car: None,
                    trip_and_person: None,
                    maybe_route: Some(route.id),
                },
                true,
            ),
        );
    }

    pub fn set_name(&mut self, name: String) {
        self.run_name = name;
    }
}

// Drawing
impl GetDrawAgents for Sim {
    fn time(&self) -> Time {
        self.time
    }

    fn step_count(&self) -> usize {
        self.step_count
    }

    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.parking.get_draw_car(id, map).or_else(|| {
            self.driving
                .get_single_draw_car(id, self.time, map, &self.transit)
        })
    }

    fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput> {
        self.walking.get_draw_ped(id, self.time, map)
    }

    fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
        let mut results = Vec::new();
        if let Traversable::Lane(l) = on {
            if map.get_l(l).is_parking() {
                return self.parking.get_draw_cars(l, map);
            }
            results.extend(self.parking.get_draw_cars_in_lots(l, map));
        }
        results.extend(
            self.driving
                .get_draw_cars_on(self.time, on, map, &self.transit),
        );
        results
    }

    fn get_draw_peds(
        &self,
        on: Traversable,
        map: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>) {
        self.walking.get_draw_peds_on(self.time, on, map)
    }

    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        let mut result = self
            .driving
            .get_all_draw_cars(self.time, map, &self.transit);
        result.extend(self.parking.get_all_draw_cars(map));
        result
    }

    fn get_all_draw_peds(&self, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_all_draw_peds(self.time, map)
    }

    fn get_unzoomed_agents(&self, map: &Map) -> Vec<UnzoomedAgent> {
        let mut result = self.driving.get_unzoomed_agents(self.time, map);
        result.extend(self.walking.get_unzoomed_agents(self.time, map));
        result
    }
}

// Running
impl Sim {
    // Advances time as minimally as possible, also limited by max_dt. Returns true if the callback
    // said to halt the sim.
    fn minimal_step(
        &mut self,
        map: &Map,
        max_dt: Duration,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
    ) -> bool {
        self.step_count += 1;

        let max_time = if let Some(t) = self.scheduler.peek_next_time() {
            if t > self.time + max_dt {
                // Next event is after when we want to stop.
                self.time += max_dt;
                return false;
            }
            t
        } else {
            // No events left at all
            self.time += max_dt;
            return false;
        };

        let mut halt = false;
        while let Some(time) = self.scheduler.peek_next_time() {
            if time > max_time {
                return false;
            }
            if let Some(cmd) = self.scheduler.get_next() {
                if self.do_step(map, time, cmd, maybe_cb) {
                    halt = true;
                    break;
                }
            }
        }

        halt
    }

    // If true, halt simulation because the callback said so.
    fn do_step(
        &mut self,
        map: &Map,
        time: Time,
        cmd: Command,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
    ) -> bool {
        self.time = time;
        let mut events = Vec::new();
        let mut halt = false;

        let mut ctx = Ctx {
            parking: &mut self.parking,
            intersections: &mut self.intersections,
            cap: &mut self.cap,
            scheduler: &mut self.scheduler,
            map,
        };

        match cmd {
            Command::StartTrip(id, trip_spec, maybe_req, maybe_path) => {
                self.trips
                    .start_trip(self.time, id, trip_spec, maybe_req, maybe_path, &mut ctx);
            }
            Command::SpawnCar(create_car, retry_if_no_room) => {
                if self.driving.start_car_on_lane(
                    self.time,
                    create_car.clone(),
                    map,
                    &self.intersections,
                    &self.parking,
                    &mut self.scheduler,
                ) {
                    if let Some((trip, person)) = create_car.trip_and_person {
                        self.trips
                            .agent_starting_trip_leg(AgentID::Car(create_car.vehicle.id), trip);
                        events.push(Event::TripPhaseStarting(
                            trip,
                            person,
                            Some(create_car.req.clone()),
                            if create_car.vehicle.id.1 == VehicleType::Car {
                                TripPhaseType::Driving
                            } else {
                                TripPhaseType::Biking
                            },
                        ));
                    }
                    if let Some(parked_car) = create_car.maybe_parked_car {
                        if let ParkingSpot::Offstreet(b, _) = parked_car.spot {
                            // Buses don't start in parking garages, so trip must exist
                            events.push(Event::PersonLeavesBuilding(
                                create_car.trip_and_person.unwrap().1,
                                b,
                            ));
                        }
                        self.parking.remove_parked_car(parked_car);
                    }
                    if let Some(route) = create_car.maybe_route {
                        self.transit.bus_created(create_car.vehicle.id, route);
                    }
                    self.analytics
                        .record_demand(create_car.router.get_path(), map);
                } else if retry_if_no_room {
                    // TODO Record this in the trip log
                    self.scheduler.push(
                        self.time + BLIND_RETRY_TO_SPAWN,
                        Command::SpawnCar(create_car, retry_if_no_room),
                    );
                } else {
                    // Buses don't use Command::SpawnCar, so this must exist.
                    let (trip, person) = create_car.trip_and_person.unwrap();
                    // Have to redeclare for the borrow checker
                    let mut ctx = Ctx {
                        parking: &mut self.parking,
                        intersections: &mut self.intersections,
                        cap: &mut self.cap,
                        scheduler: &mut self.scheduler,
                        map,
                    };
                    self.trips.cancel_trip(
                        self.time,
                        trip,
                        format!(
                            "no room to spawn car for {} by {}, not retrying",
                            trip, person
                        ),
                        Some(create_car.vehicle),
                        &mut ctx,
                    );
                }
            }
            Command::SpawnPed(create_ped) => {
                // Do the order a bit backwards so we don't have to clone the
                // CreatePedestrian. spawn_ped can't fail.
                self.trips
                    .agent_starting_trip_leg(AgentID::Pedestrian(create_ped.id), create_ped.trip);
                events.push(Event::TripPhaseStarting(
                    create_ped.trip,
                    create_ped.person,
                    Some(create_ped.req.clone()),
                    TripPhaseType::Walking,
                ));
                self.analytics.record_demand(&create_ped.path, map);

                // Maybe there's actually no work to do!
                match (&create_ped.start.connection, &create_ped.goal.connection) {
                    (
                        SidewalkPOI::Building(b1),
                        SidewalkPOI::ParkingSpot(ParkingSpot::Offstreet(b2, idx)),
                    ) if b1 == b2 => {
                        self.trips.ped_reached_parking_spot(
                            self.time,
                            create_ped.id,
                            ParkingSpot::Offstreet(*b2, *idx),
                            Duration::ZERO,
                            &mut ctx,
                        );
                    }
                    _ => {
                        if let SidewalkPOI::Building(b) = &create_ped.start.connection {
                            events.push(Event::PersonLeavesBuilding(create_ped.person, *b));
                        }

                        self.walking
                            .spawn_ped(self.time, create_ped, map, &mut self.scheduler);
                    }
                }
            }
            Command::UpdateCar(car) => {
                self.driving.update_car(
                    car,
                    self.time,
                    &mut ctx,
                    &mut self.trips,
                    &mut self.transit,
                    &mut self.walking,
                );
            }
            Command::UpdateLaggyHead(car) => {
                self.driving.update_laggy_head(car, self.time, &mut ctx);
            }
            Command::UpdatePed(ped) => {
                self.walking.update_ped(
                    ped,
                    self.time,
                    &mut ctx,
                    &mut self.trips,
                    &mut self.transit,
                );
            }
            Command::UpdateIntersection(i) => {
                self.intersections
                    .update_intersection(self.time, i, map, &mut self.scheduler);
            }
            Command::Callback(frequency) => {
                self.scheduler
                    .push(self.time + frequency, Command::Callback(frequency));
                if maybe_cb.as_mut().unwrap().run(self, map) {
                    halt = true;
                }
            }
            Command::Pandemic(cmd) => {
                self.pandemic
                    .as_mut()
                    .unwrap()
                    .handle_cmd(self.time, cmd, &mut self.scheduler);
            }
            Command::FinishRemoteTrip(trip) => {
                self.trips.remote_trip_finished(self.time, trip, &mut ctx);
            }
            Command::StartBus(r, _) => {
                self.start_bus(map.get_br(r), map);
            }
        }

        // Record events at precisely the time they occur.
        self.dispatch_events(events, map);

        halt
    }

    fn dispatch_events(&mut self, mut events: Vec<Event>, map: &Map) {
        events.extend(self.trips.collect_events());
        events.extend(self.transit.collect_events());
        events.extend(self.driving.collect_events());
        events.extend(self.walking.collect_events());
        events.extend(self.intersections.collect_events());
        events.extend(self.parking.collect_events());
        for ev in events {
            if let Some(ref mut m) = self.pandemic {
                m.handle_event(self.time, &ev, &mut self.scheduler);
            }

            self.analytics.event(ev, self.time, map);
        }
    }

    pub fn timed_step(
        &mut self,
        map: &Map,
        dt: Duration,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
        timer: &mut Timer,
    ) {
        let end_time = self.time + dt;
        let start = Instant::now();
        let mut last_update = Instant::now();

        timer.start(format!("Advance sim to {}", end_time));
        while self.time < end_time {
            if self.minimal_step(map, end_time - self.time, maybe_cb) {
                break;
            }
            if !self.analytics.alerts.is_empty() {
                match self.alerts {
                    AlertHandler::Print => {
                        for (t, loc, msg) in self.analytics.alerts.drain(..) {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                    }
                    AlertHandler::Block => {
                        for (t, loc, msg) in &self.analytics.alerts {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                        break;
                    }
                    AlertHandler::Silence => {
                        self.analytics.alerts.clear();
                    }
                }
            }
            if Duration::realtime_elapsed(last_update) >= Duration::seconds(1.0) {
                // TODO Not timer?
                println!(
                    "- After {}, the sim is at {}. {} live agents",
                    Duration::realtime_elapsed(start),
                    self.time,
                    prettyprint_usize(self.trips.num_active_agents()),
                );
                last_update = Instant::now();
            }
        }
        timer.stop(format!("Advance sim to {}", end_time));
    }
    pub fn tiny_step(&mut self, map: &Map, maybe_cb: &mut Option<Box<dyn SimCallback>>) {
        self.timed_step(
            map,
            Duration::seconds(0.1),
            maybe_cb,
            &mut Timer::throwaway(),
        );
    }

    pub fn time_limited_step(
        &mut self,
        map: &Map,
        dt: Duration,
        real_time_limit: Duration,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
    ) {
        let started_at = Instant::now();
        let end_time = self.time + dt;

        while self.time < end_time && Duration::realtime_elapsed(started_at) < real_time_limit {
            if self.minimal_step(map, end_time - self.time, maybe_cb) {
                break;
            }
            if !self.analytics.alerts.is_empty() {
                match self.alerts {
                    AlertHandler::Print => {
                        for (t, loc, msg) in self.analytics.alerts.drain(..) {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                    }
                    AlertHandler::Block => {
                        for (t, loc, msg) in &self.analytics.alerts {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                        break;
                    }
                    AlertHandler::Silence => {
                        self.analytics.alerts.clear();
                    }
                }
            }
        }
    }

    pub fn dump_before_abort(&self) {
        println!("At {}", self.time);
        if let Some(path) = self.find_previous_savestate(self.time) {
            println!("Debug from {}", path);
        }
    }
}

// Helpers to run the sim
// TODO Old and gunky
impl Sim {
    pub fn run_until_done<F: Fn(&mut Sim, &Map)>(
        &mut self,
        map: &Map,
        callback: F,
        // Interpreted as a relative time
        time_limit: Option<Duration>,
    ) {
        let mut last_print = Instant::now();
        let mut last_sim_time = self.time();

        loop {
            // TODO Regular printing doesn't happen if we use a time_limit :\
            let dt = time_limit.unwrap_or_else(|| Duration::seconds(30.0));

            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.timed_step(map, dt, &mut None, &mut Timer::throwaway());
            })) {
                Ok(()) => {}
                Err(err) => {
                    println!(
                        "*************************************************************************\
                         *******"
                    );
                    println!("Sim broke:");
                    self.dump_before_abort();
                    panic::resume_unwind(err);
                }
            }

            let dt_real = Duration::realtime_elapsed(last_print);
            if dt_real >= Duration::seconds(1.0) {
                let (finished, unfinished) = self.num_trips();
                println!(
                    "{}: {} trips finished, {} unfinished, speed = {:.2}x, {}",
                    self.time(),
                    prettyprint_usize(finished),
                    prettyprint_usize(unfinished),
                    (self.time() - last_sim_time) / dt_real,
                    self.scheduler.describe_stats()
                );
                last_print = Instant::now();
                last_sim_time = self.time();
            }
            callback(self, map);
            if self.is_done() {
                println!(
                    "{}: speed = {:.2}x, {}",
                    self.time(),
                    (self.time() - last_sim_time) / dt_real,
                    self.scheduler.describe_stats()
                );
                break;
            }

            if let Some(lim) = time_limit {
                panic!("Time limit {} hit", lim);
            }
        }
    }
}

// Savestating
impl Sim {
    pub fn save_dir(&self) -> String {
        abstutil::path_all_saves(&self.map_name, &self.edits_name, &self.run_name)
    }

    fn save_path(&self, base_time: Time) -> String {
        // If we wanted to be even more reproducible, we'd encode RNG seed, version of code, etc,
        // but that's overkill right now.
        abstutil::path_save(
            &self.map_name,
            &self.edits_name,
            &self.run_name,
            base_time.as_filename(),
        )
    }

    pub fn save(&mut self) -> String {
        let restore = self.scheduler.before_savestate();

        if true {
            println!("sim savestate breakdown:");
            println!(
                "- driving: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.driving))
            );
            println!(
                "- parking: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.parking))
            );
            println!(
                "- walking: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.walking))
            );
            println!(
                "- intersections: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.intersections))
            );
            println!(
                "- transit: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.transit))
            );
            println!(
                "- cap: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.cap))
            );
            println!(
                "- trips: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.trips))
            );
            println!(
                "- scheduler: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.scheduler))
            );
        }

        let path = self.save_path(self.time);
        abstutil::write_binary(path.clone(), self);

        self.scheduler.after_savestate(restore);

        path
    }

    pub fn find_previous_savestate(&self, base_time: Time) -> Option<String> {
        abstutil::find_prev_file(self.save_path(base_time))
    }

    pub fn find_next_savestate(&self, base_time: Time) -> Option<String> {
        abstutil::find_next_file(self.save_path(base_time))
    }

    pub fn load_savestate(
        path: String,
        map: &Map,
        timer: &mut Timer,
    ) -> Result<Sim, std::io::Error> {
        let mut sim: Sim = abstutil::maybe_read_binary(path, timer)?;
        sim.restore_paths(map, timer);
        Ok(sim)
    }

    pub fn restore_paths(&mut self, map: &Map, timer: &mut Timer) {
        let paths = timer.parallelize(
            "calculate paths",
            Parallelism::Fastest,
            self.scheduler.get_requests_for_savestate(),
            |req| map.pathfind(req).unwrap(),
        );
        self.scheduler.after_savestate(paths);
    }
}

// Live edits
impl Sim {
    pub fn handle_live_edited_traffic_signals(&mut self, map: &Map) {
        self.intersections
            .handle_live_edited_traffic_signals(self.time, map, &mut self.scheduler)
    }

    pub fn handle_live_edits(&mut self, map: &Map) {
        let affected = self.find_trips_affected_by_live_edits(map);

        // V1: Just cancel every trip crossing an affected area.
        // (V2 is probably rerouting everyone, only cancelling when that fails)
        // TODO If we delete a bus, deal with all its passengers
        let mut ctx = Ctx {
            parking: &mut self.parking,
            intersections: &mut self.intersections,
            cap: &mut self.cap,
            scheduler: &mut self.scheduler,
            map,
        };
        for (agent, trip) in affected {
            match agent {
                AgentID::Car(car) => {
                    let vehicle = self.driving.delete_car(car, self.time, &mut ctx);
                    // TODO Plumb more info about the reason
                    self.trips.cancel_trip(
                        self.time,
                        trip,
                        format!("map edited without reset"),
                        Some(vehicle),
                        &mut ctx,
                    );
                }
                AgentID::Pedestrian(ped) => {
                    self.walking.delete_ped(ped, ctx.scheduler);
                    self.trips.cancel_trip(
                        self.time,
                        trip,
                        format!("map edited without reset"),
                        None,
                        &mut ctx,
                    );
                }
                AgentID::BusPassenger(_, _) => unreachable!(),
            }
        }
    }

    fn find_trips_affected_by_live_edits(&mut self, map: &Map) -> Vec<(AgentID, TripID)> {
        let mut affected: Vec<(AgentID, TripID)> = Vec::new();

        // TODO Handle changes to access restrictions

        {
            // Find every active trip whose path crosses a modified lane or closed intersection
            let (edited_lanes, _) = map.get_edits().changed_lanes(map);
            let mut closed_intersections = HashSet::new();
            for i in map.get_edits().original_intersections.keys() {
                if map.get_i(*i).is_closed() {
                    closed_intersections.insert(*i);
                }
            }
            for (a, trip) in self.trips.active_agents_and_trips() {
                if let Some(path) = self.get_path(*a) {
                    if path
                        .get_steps()
                        .iter()
                        .any(|step| match step.as_traversable() {
                            Traversable::Lane(l) => edited_lanes.contains(&l),
                            Traversable::Turn(t) => closed_intersections.contains(&t.parent),
                        })
                    {
                        affected.push((*a, *trip));
                    }
                }
            }
        }

        {
            let evicted_cars = self.parking.handle_live_edits(map, &mut Timer::throwaway());
            affected.extend(self.walking.find_trips_to_parking(evicted_cars));

            if !self.parking.is_infinite() {
                let (filled, avail) = self.parking.get_all_parking_spots();
                let mut all_spots: BTreeSet<ParkingSpot> = BTreeSet::new();
                all_spots.extend(filled);
                all_spots.extend(avail);
                affected.extend(self.driving.find_trips_to_edited_parking(all_spots));
            }
        }

        affected
    }
}

// Invasive debugging
impl Sim {
    pub fn delete_car(&mut self, id: CarID, map: &Map) {
        if let Some(trip) = self.agent_to_trip(AgentID::Car(id)) {
            let mut ctx = Ctx {
                parking: &mut self.parking,
                intersections: &mut self.intersections,
                cap: &mut self.cap,
                scheduler: &mut self.scheduler,
                map,
            };
            let vehicle = self.driving.delete_car(id, self.time, &mut ctx);
            self.trips.cancel_trip(
                self.time,
                trip,
                format!("{} deleted manually through the UI", id),
                Some(vehicle),
                &mut ctx,
            );
        } else {
            println!("{} has no trip?!", id);
        }
    }

    pub fn clear_alerts(&mut self) -> Vec<(Time, AlertLocation, String)> {
        std::mem::replace(&mut self.analytics.alerts, Vec::new())
    }
}

// Callbacks
pub trait SimCallback: downcast_rs::Downcast {
    // Run at some scheduled time. If this returns true, halt simulation.
    fn run(&mut self, sim: &Sim, map: &Map) -> bool;
}
downcast_rs::impl_downcast!(SimCallback);

impl Sim {
    // Only one at a time supported.
    pub fn set_periodic_callback(&mut self, frequency: Duration) {
        // TODO Round up time nicely?
        self.scheduler
            .push(self.time + frequency, Command::Callback(frequency));
    }
    pub fn unset_periodic_callback(&mut self) {
        // Frequency doesn't matter
        self.scheduler
            .cancel(Command::Callback(Duration::seconds(1.0)));
    }
}
