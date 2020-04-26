use crate::{
    AgentID, AlertLocation, Analytics, CarID, Command, CreateCar, DrawCarInput, DrawPedCrowdInput,
    DrawPedestrianInput, DrivingSimState, Event, GetDrawAgents, IntersectionSimState,
    PandemicModel, ParkedCar, ParkingSimState, ParkingSpot, PedestrianID, Person, PersonID,
    PersonState, Router, Scheduler, SidewalkPOI, SidewalkSpot, TransitSimState, TripEndpoint,
    TripID, TripManager, TripMode, TripPhaseType, TripPositions, TripResult, TripSpawner,
    UnzoomedAgent, Vehicle, VehicleSpec, VehicleType, WalkingSimState, BUS_LENGTH, MIN_CAR_LENGTH,
};
use abstutil::Timer;
use derivative::Derivative;
use geom::{Distance, Duration, PolyLine, Pt2D, Speed, Time};
use instant::Instant;
use map_model::{
    BuildingID, BusRoute, BusRouteID, IntersectionID, LaneID, Map, Path, PathConstraints,
    PathRequest, PathStep, Position, RoadID, Traversable,
};
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::panic;

// TODO Do something else.
const BLIND_RETRY_TO_SPAWN: Duration = Duration::const_seconds(5.0);

#[derive(Serialize, Deserialize, Clone, Derivative)]
#[derivative(PartialEq)]
pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    transit: TransitSimState,
    trips: TripManager,
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    pandemic: Option<PandemicModel>,
    scheduler: Scheduler,
    time: Time,

    // TODO Reconsider these
    pub(crate) map_name: String,
    pub(crate) edits_name: String,
    // Some tests deliberately set different scenario names for comparisons.
    // TODO Maybe get rid of this, now that savestates aren't used
    #[derivative(PartialEq = "ignore")]
    run_name: String,
    #[derivative(PartialEq = "ignore")]
    step_count: usize,

    // Lazily computed.
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    trip_positions: Option<TripPositions>,
    // Don't serialize, to reduce prebaked savestate size. Analytics are saved once covering the
    // full day and can be trimmed to any time.
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    analytics: Analytics,

    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    check_for_gridlock: Option<(Time, Duration)>,

    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    alerts: AlertHandler,
}

#[derive(Clone)]
pub struct SimOptions {
    pub run_name: String,
    pub savestate_every: Option<Duration>,
    pub use_freeform_policy_everywhere: bool,
    pub disable_block_the_box: bool,
    pub recalc_lanechanging: bool,
    pub break_turn_conflict_cycles: bool,
    pub enable_pandemic_model: Option<XorShiftRng>,
    pub alerts: AlertHandler,
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
            savestate_every: None,
            use_freeform_policy_everywhere: false,
            disable_block_the_box: false,
            recalc_lanechanging: true,
            break_turn_conflict_cycles: false,
            enable_pandemic_model: None,
            alerts: AlertHandler::Print,
        }
    }
}

// Setup
impl Sim {
    pub fn new(map: &Map, opts: SimOptions, timer: &mut Timer) -> Sim {
        let mut scheduler = Scheduler::new();
        if let Some(d) = opts.savestate_every {
            scheduler.push(Time::START_OF_DAY + d, Command::Savestate(d));
        }
        Sim {
            driving: DrivingSimState::new(map, opts.recalc_lanechanging),
            parking: ParkingSimState::new(map, timer),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(
                map,
                &mut scheduler,
                opts.use_freeform_policy_everywhere,
                opts.disable_block_the_box,
                opts.break_turn_conflict_cycles,
            ),
            transit: TransitSimState::new(),
            trips: TripManager::new(),
            pandemic: if let Some(rng) = opts.enable_pandemic_model {
                Some(PandemicModel::new(rng))
            } else {
                None
            },
            scheduler,
            time: Time::START_OF_DAY,

            map_name: map.get_name().to_string(),
            // TODO
            edits_name: "untitled edits".to_string(),
            run_name: opts.run_name,
            step_count: 0,
            trip_positions: None,
            check_for_gridlock: None,
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

    // (Filled, available)
    pub fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        self.parking.get_all_parking_spots()
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
        let spot = if let Some((spot, _)) = self.parking.get_first_free_spot(
            Position::new(driving_lane, Distance::ZERO),
            &vehicle,
            map,
        ) {
            spot
        } else {
            let (_, spot, _) =
                self.parking
                    .path_to_free_parking_spot(driving_lane, &vehicle, map)?;
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
    pub fn new_person(&mut self, p: PersonID, ped_speed: Speed, vehicle_specs: Vec<VehicleSpec>) {
        self.trips.new_person(p, ped_speed, vehicle_specs);
    }
    pub fn random_person(&mut self, ped_speed: Speed, vehicle_specs: Vec<VehicleSpec>) -> &Person {
        self.trips.random_person(ped_speed, vehicle_specs)
    }
    pub(crate) fn seed_parked_car(&mut self, vehicle: Vehicle, spot: ParkingSpot) {
        self.parking.reserve_spot(spot);
        self.parking.add_parked_car(ParkedCar { vehicle, spot });
    }

    pub fn get_offstreet_parked_cars(&self, bldg: BuildingID) -> Vec<&ParkedCar> {
        self.parking.get_offstreet_parked_cars(bldg)
    }

    pub fn seed_bus_route(&mut self, route: &BusRoute, map: &Map, timer: &mut Timer) -> Vec<CarID> {
        let mut results: Vec<CarID> = Vec::new();

        // Try to spawn just ONE bus anywhere.
        // TODO Be more realistic. One bus per stop is too much, one is too little.
        for (next_stop_idx, req, mut path, end_dist) in
            self.transit.create_empty_route(route, map).into_iter()
        {
            // For now, no desire for randomness. Caller can pass in list of specs if that ever
            // changes.
            let vehicle = VehicleSpec {
                vehicle_type: VehicleType::Bus,
                length: BUS_LENGTH,
                max_speed: None,
            }
            .make(CarID(self.trips.new_car_id(), VehicleType::Bus), None);
            let id = vehicle.id;

            loop {
                if path.is_last_step() {
                    timer.warn(format!(
                        "Giving up on seeding a bus headed towards stop {} of {} ({})",
                        next_stop_idx, route.name, route.id
                    ));
                    break;
                }
                let start_lane = if let PathStep::Lane(l) = path.current_step() {
                    l
                } else {
                    path.shift(map);
                    // TODO Technically should update request, but it shouldn't matter
                    continue;
                };
                if map.get_l(start_lane).length() < vehicle.length {
                    path.shift(map);
                    // TODO Technically should update request, but it shouldn't matter
                    continue;
                }

                // Bypass some layers of abstraction that don't make sense for buses.
                if self.driving.start_car_on_lane(
                    self.time,
                    CreateCar {
                        start_dist: vehicle.length,
                        vehicle: vehicle.clone(),
                        req: req.clone(),
                        router: Router::follow_bus_route(path.clone(), end_dist),
                        maybe_parked_car: None,
                        trip_and_person: None,
                    },
                    map,
                    &self.intersections,
                    &self.parking,
                    &mut self.scheduler,
                ) {
                    self.transit.bus_created(id, route.id, next_stop_idx);
                    self.analytics.record_demand(&path, map);
                    results.push(id);
                    return results;
                } else {
                    path.shift(map);
                }
            }
        }
        if results.is_empty() {
            // TODO Bigger failure
            timer.warn(format!("Failed to make ANY buses for {}!", route.name));
        }
        results
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
        if let Traversable::Lane(l) = on {
            if map.get_l(l).is_parking() {
                return self.parking.get_draw_cars(l, map);
            }
        }
        self.driving
            .get_draw_cars_on(self.time, on, map, &self.transit)
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
    // Advances time as minimally as possible, also limited by max_dt.
    fn minimal_step(&mut self, map: &Map, max_dt: Duration) {
        self.step_count += 1;

        let max_time = if let Some(t) = self.scheduler.peek_next_time() {
            if t > self.time + max_dt {
                // Next event is after when we want to stop.
                self.time += max_dt;
                return;
            }
            t
        } else {
            // No events left at all
            self.time += max_dt;
            return;
        };

        let mut savestate = false;
        while let Some(time) = self.scheduler.peek_next_time() {
            if time > max_time {
                return;
            }
            if let Some(cmd) = self.scheduler.get_next() {
                if self.do_step(map, time, cmd) {
                    savestate = true;
                }
            }
        }

        self.trip_positions = None;

        if savestate {
            self.save();
        }
    }

    // If true, savestate was requested.
    fn do_step(&mut self, map: &Map, time: Time, cmd: Command) -> bool {
        self.time = time;
        let mut events = Vec::new();
        let mut savestate = false;
        match cmd {
            Command::StartTrip(id, trip_spec, maybe_req, maybe_path) => {
                self.trips.start_trip(
                    self.time,
                    id,
                    trip_spec,
                    maybe_req,
                    maybe_path,
                    &mut self.parking,
                    &mut self.scheduler,
                    map,
                );
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
                    if let Some((trip, _)) = create_car.trip_and_person {
                        self.trips
                            .agent_starting_trip_leg(AgentID::Car(create_car.vehicle.id), trip);
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
                    if let Some((trip, person)) = create_car.trip_and_person {
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
                    println!(
                        "No room to spawn car for {} by {}. Not retrying!",
                        trip, person
                    );
                    self.trips.abort_trip(
                        self.time,
                        trip,
                        Some(create_car.vehicle),
                        &mut self.parking,
                        &mut self.scheduler,
                        map,
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
                        events.push(Event::Alert(
                            AlertLocation::Building(*b1),
                            format!("car leaving bldg"),
                        ));
                        self.trips.ped_reached_parking_spot(
                            self.time,
                            create_ped.id,
                            ParkingSpot::Offstreet(*b2, *idx),
                            Duration::ZERO,
                            map,
                            &mut self.parking,
                            &mut self.scheduler,
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
                    map,
                    &mut self.parking,
                    &mut self.intersections,
                    &mut self.trips,
                    &mut self.scheduler,
                    &mut self.transit,
                    &mut self.walking,
                );
            }
            Command::UpdateLaggyHead(car) => {
                self.driving.update_laggy_head(
                    car,
                    self.time,
                    map,
                    &mut self.intersections,
                    &mut self.scheduler,
                );
            }
            Command::UpdatePed(ped) => {
                self.walking.update_ped(
                    ped,
                    self.time,
                    map,
                    &mut self.intersections,
                    &mut self.parking,
                    &mut self.scheduler,
                    &mut self.trips,
                    &mut self.transit,
                );
            }
            Command::UpdateIntersection(i) => {
                self.intersections
                    .update_intersection(self.time, i, map, &mut self.scheduler);
            }
            Command::Savestate(frequency) => {
                self.scheduler
                    .push(self.time + frequency, Command::Savestate(frequency));
                savestate = true;
            }
            Command::Pandemic(cmd) => {
                self.pandemic
                    .as_mut()
                    .unwrap()
                    .handle_cmd(self.time, cmd, &mut self.scheduler);
            }
            Command::FinishRemoteTrip(trip) => {
                self.trips.remote_trip_finished(
                    self.time,
                    trip,
                    map,
                    &mut self.parking,
                    &mut self.scheduler,
                );
            }
        }

        // Record events at precisely the time they occur.
        self.dispatch_events(events, map);

        savestate
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

    pub fn timed_step(&mut self, map: &Map, dt: Duration, timer: &mut Timer) {
        let end_time = self.time + dt;
        let start = Instant::now();
        let mut last_update = Instant::now();

        timer.start(format!("Advance sim to {}", end_time));
        while self.time < end_time {
            self.minimal_step(map, end_time - self.time);
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
                    "- After {}, the sim is at {}",
                    Duration::realtime_elapsed(start),
                    self.time
                );
                last_update = Instant::now();
            }
        }
        timer.stop(format!("Advance sim to {}", end_time));
    }
    pub fn normal_step(&mut self, map: &Map, dt: Duration) {
        self.timed_step(map, dt, &mut Timer::throwaway());
    }

    // TODO Do this like periodic savestating instead?
    pub fn set_gridlock_checker(&mut self, freq: Option<Duration>) {
        if let Some(dt) = freq {
            assert!(self.check_for_gridlock.is_none());
            self.check_for_gridlock = Some((self.time + dt, dt));
        } else {
            assert!(self.check_for_gridlock.is_some());
            self.check_for_gridlock = None;
        }
    }
    // This will return delayed intersections if that's why it stops early.
    pub fn time_limited_step(
        &mut self,
        map: &Map,
        dt: Duration,
        real_time_limit: Duration,
    ) -> Option<Vec<(IntersectionID, Time)>> {
        let started_at = Instant::now();
        let end_time = self.time + dt;

        while self.time < end_time && Duration::realtime_elapsed(started_at) < real_time_limit {
            self.minimal_step(map, end_time - self.time);
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
            if let Some((ref mut t, dt)) = self.check_for_gridlock {
                if self.time >= *t {
                    *t += dt;
                    let gridlock = self.delayed_intersections(dt);
                    if !gridlock.is_empty() {
                        return Some(gridlock);
                    }
                }
            }
        }

        None
    }

    pub fn dump_before_abort(&self) {
        println!(
            "********************************************************************************"
        );
        println!("At {}", self.time);
        if let Some(path) = self.find_previous_savestate(self.time) {
            println!("Debug from {}", path);
        }
    }
}

// Helpers to run the sim
// TODO Old and gunky
impl Sim {
    pub fn just_run_until_done(&mut self, map: &Map, time_limit: Option<Duration>) {
        self.run_until_done(map, |_, _| {}, time_limit);
    }

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
                self.normal_step(&map, dt);
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
                let (finished, unfinished, _) = self.num_trips();
                println!(
                    "{}: {} trips finished, {} unfinished, speed = {:.2}x, {}",
                    self.time(),
                    abstutil::prettyprint_usize(finished),
                    abstutil::prettyprint_usize(unfinished),
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

    pub fn run_until_expectations_met(
        &mut self,
        map: &Map,
        all_expectations: Vec<Event>,
        // Interpreted as a relative time
        time_limit: Duration,
    ) {
        // TODO No benchmark printing at all this way.
        // TODO Doesn't stop early once all expectations are met.
        self.analytics.test_expectations.extend(all_expectations);
        self.normal_step(&map, time_limit);
        if self.analytics.test_expectations.is_empty() {
            return;
        }
        panic!(
            "Time limit {} hit, but some expectations never met: {:?}",
            time_limit, self.analytics.test_expectations
        );
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
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.driving))
            );
            println!(
                "- parking: {} bytes",
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.parking))
            );
            println!(
                "- walking: {} bytes",
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.walking))
            );
            println!(
                "- intersections: {} bytes",
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.intersections))
            );
            println!(
                "- transit: {} bytes",
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.transit))
            );
            println!(
                "- trips: {} bytes",
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.trips))
            );
            println!(
                "- scheduler: {} bytes",
                abstutil::prettyprint_usize(abstutil::serialized_size_bytes(&self.scheduler))
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
            self.scheduler.get_requests_for_savestate(),
            |req| map.pathfind(req).unwrap(),
        );
        self.scheduler.after_savestate(paths);
    }
}

// Queries of all sorts
impl Sim {
    pub fn time(&self) -> Time {
        self.time
    }

    pub fn is_done(&self) -> bool {
        self.trips.is_done()
    }

    pub fn is_empty(&self) -> bool {
        self.time == Time::START_OF_DAY && self.is_done()
    }

    // (number of finished trips, number of unfinished trips, number of active by mode)
    pub fn num_trips(&self) -> (usize, usize, BTreeMap<TripMode, usize>) {
        self.trips.num_trips()
    }
    // (total number of people, just in buildings, just off map)
    pub fn num_ppl(&self) -> (usize, usize, usize) {
        self.trips.num_ppl()
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        self.walking.debug_ped(id);
        self.trips.debug_trip(AgentID::Pedestrian(id));
    }

    pub fn debug_car(&self, id: CarID) {
        self.driving.debug_car(id);
        self.trips.debug_trip(AgentID::Car(id));
    }

    pub fn debug_intersection(&self, id: IntersectionID, map: &Map) {
        self.intersections.debug(id, map);
    }

    pub fn debug_lane(&self, id: LaneID) {
        self.driving.debug_lane(id);
    }

    // Only call for active agents, will panic otherwise
    pub fn agent_properties(&self, id: AgentID) -> AgentProperties {
        match id {
            AgentID::Pedestrian(id) => self.walking.agent_properties(id, self.time),
            AgentID::Car(id) => self.driving.agent_properties(id, self.time),
        }
    }

    // TODO Temporary until we figure out all the info to expose
    pub fn bus_properties(&self, car: CarID, map: &Map) -> Vec<(String, String)> {
        let passengers = self.transit.get_passengers(car);
        vec![
            (
                "Route".to_string(),
                map.get_br(self.transit.bus_route(car)).name.clone(),
            ),
            ("Passengers".to_string(), passengers.len().to_string()),
        ]
    }

    pub fn bus_route_id(&self, maybe_bus: CarID) -> Option<BusRouteID> {
        if maybe_bus.1 == VehicleType::Bus {
            Some(self.transit.bus_route(maybe_bus))
        } else {
            None
        }
    }

    pub fn active_agents(&self) -> Vec<AgentID> {
        self.trips.active_agents()
    }

    pub fn agent_to_trip(&self, id: AgentID) -> Option<TripID> {
        self.trips.agent_to_trip(id)
    }

    pub fn trip_to_agent(&self, id: TripID) -> TripResult<AgentID> {
        self.trips.trip_to_agent(id)
    }

    // (start time, start position, end position, trip type)
    pub fn trip_info(&self, id: TripID) -> (Time, TripEndpoint, TripEndpoint, TripMode) {
        self.trips.trip_info(id)
    }
    // If trip is finished, returns (total time, total waiting time)
    pub fn finished_trip_time(&self, id: TripID) -> Option<(Duration, Duration)> {
        self.trips.finished_trip_time(id)
    }

    pub fn trip_to_person(&self, id: TripID) -> PersonID {
        self.trips.trip_to_person(id)
    }
    // TODO This returns None for parked cars owned by people! That's confusing. Dedupe with
    // get_owner_of_car.
    pub fn agent_to_person(&self, id: AgentID) -> Option<PersonID> {
        self.agent_to_trip(id).map(|t| self.trip_to_person(t))
    }
    pub fn get_owner_of_car(&self, id: CarID) -> Option<PersonID> {
        self.driving
            .get_owner_of_car(id)
            .or_else(|| self.parking.get_owner_of_car(id))
    }
    pub fn lookup_parked_car(&self, id: CarID) -> Option<&ParkedCar> {
        self.parking.lookup_parked_car(id)
    }

    pub fn lookup_person(&self, id: PersonID) -> Option<&Person> {
        self.trips.get_person(id)
    }
    pub fn get_person(&self, id: PersonID) -> &Person {
        self.trips.get_person(id).unwrap()
    }
    pub fn get_all_people(&self) -> &Vec<Person> {
        self.trips.get_all_people()
    }

    pub fn lookup_car_id(&self, idx: usize) -> Option<CarID> {
        for vt in &[VehicleType::Car, VehicleType::Bike, VehicleType::Bus] {
            let id = CarID(idx, *vt);
            if self.driving.does_car_exist(id) {
                return Some(id);
            }
        }

        let id = CarID(idx, VehicleType::Car);
        // Only cars can be parked.
        if self.parking.lookup_parked_car(id).is_some() {
            return Some(id);
        }

        None
    }

    pub fn get_path(&self, id: AgentID) -> Option<&Path> {
        match id {
            AgentID::Car(car) => self.driving.get_path(car),
            AgentID::Pedestrian(ped) => self.walking.get_path(ped),
        }
    }

    pub fn trace_route(
        &self,
        id: AgentID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
        match id {
            AgentID::Car(car) => self.driving.trace_route(self.time, car, map, dist_ahead),
            AgentID::Pedestrian(ped) => self.walking.trace_route(self.time, ped, map, dist_ahead),
        }
    }

    pub fn get_trip_positions(&mut self, map: &Map) -> &TripPositions {
        if self.trip_positions.is_some() {
            return self.trip_positions.as_ref().unwrap();
        }

        let mut trip_positions = TripPositions::new(self.time);
        self.driving
            .populate_trip_positions(&mut trip_positions, map);
        self.walking
            .populate_trip_positions(&mut trip_positions, map);

        self.trip_positions = Some(trip_positions);
        self.trip_positions.as_ref().unwrap()
    }

    pub fn get_canonical_pt_per_trip(&self, trip: TripID, map: &Map) -> TripResult<Pt2D> {
        let agent = match self.trips.trip_to_agent(trip) {
            TripResult::Ok(a) => a,
            x => {
                return x.propagate_error();
            }
        };
        if let Some(pt) = self.canonical_pt_for_agent(agent, map) {
            return TripResult::Ok(pt);
        }
        TripResult::ModeChange
    }
    pub fn get_canonical_pt_per_person(&self, p: PersonID, map: &Map) -> Option<Pt2D> {
        match self.trips.get_person(p)?.state {
            PersonState::Inside(b) => Some(map.get_b(b).polygon.center()),
            PersonState::Trip(t) => self.get_canonical_pt_per_trip(t, map).ok(),
            PersonState::OffMap => None,
        }
    }

    pub fn does_agent_exist(&self, id: AgentID) -> bool {
        match id {
            AgentID::Car(id) => {
                self.parking.lookup_parked_car(id).is_some() || self.driving.does_car_exist(id)
            }
            AgentID::Pedestrian(id) => self.walking.does_ped_exist(id),
        }
    }

    pub fn canonical_pt_for_agent(&self, id: AgentID, map: &Map) -> Option<Pt2D> {
        match id {
            AgentID::Car(id) => self
                .parking
                .canonical_pt(id, map)
                .or_else(|| Some(self.get_draw_car(id, map)?.body.last_pt())),
            AgentID::Pedestrian(id) => Some(self.get_draw_ped(id, map)?.pos),
        }
    }

    pub fn get_accepted_agents(&self, id: IntersectionID) -> HashSet<AgentID> {
        self.intersections.get_accepted_agents(id)
    }

    pub fn location_of_buses(&self, route: BusRouteID, map: &Map) -> Vec<(CarID, Pt2D)> {
        let mut results = Vec::new();
        for (car, _) in self.transit.buses_for_route(route) {
            // TODO This is a slow, indirect method!
            results.push((
                car,
                self.canonical_pt_for_agent(AgentID::Car(car), map).unwrap(),
            ));
        }
        results
    }

    // (bus, stop index it's coming from, percent to next stop)
    pub fn status_of_buses(&self, route: BusRouteID) -> Vec<(CarID, usize, f64)> {
        let mut results = Vec::new();
        for (bus, stop_idx) in self.transit.buses_for_route(route) {
            results.push((bus, stop_idx, self.driving.percent_along_route(bus)));
        }
        results
    }

    pub fn get_analytics(&self) -> &Analytics {
        &self.analytics
    }

    pub fn find_blockage_front(&self, car: CarID, map: &Map) -> String {
        self.driving
            .find_blockage_front(car, map, &self.intersections)
    }

    // For intersections with an agent waiting beyond some threshold, return when they started
    // waiting. Sorted by earliest waiting (likely the root cause of gridlock).
    pub fn delayed_intersections(&self, threshold: Duration) -> Vec<(IntersectionID, Time)> {
        self.intersections.find_gridlock(self.time, threshold)
    }

    pub fn bldg_to_people(&self, b: BuildingID) -> Vec<PersonID> {
        self.trips.bldg_to_people(b)
    }

    pub fn worst_delay(
        &self,
        map: &Map,
    ) -> (
        BTreeMap<RoadID, Duration>,
        BTreeMap<IntersectionID, Duration>,
    ) {
        self.intersections.worst_delay(self.time, map)
    }

    pub fn get_pandemic_model(&self) -> Option<&PandemicModel> {
        self.pandemic.as_ref()
    }

    pub fn get_end_of_day(&self) -> Time {
        // Always count at least 24 hours
        self.scheduler
            .get_last_time()
            .max(Time::START_OF_DAY + Duration::hours(24))
    }
}

// Invasive debugging
impl Sim {
    pub fn kill_stuck_car(&mut self, id: CarID, map: &Map) {
        if let Some(trip) = self.agent_to_trip(AgentID::Car(id)) {
            let vehicle = self.driving.kill_stuck_car(
                id,
                self.time,
                map,
                &mut self.scheduler,
                &mut self.intersections,
            );
            self.trips.abort_trip(
                self.time,
                trip,
                Some(vehicle),
                &mut self.parking,
                &mut self.scheduler,
                map,
            );
            println!("Forcibly killed {}", id);
        } else {
            println!("{} has no trip?!", id);
        }
    }

    pub fn clear_alerts(&mut self) -> Vec<(Time, AlertLocation, String)> {
        std::mem::replace(&mut self.analytics.alerts, Vec::new())
    }
}

pub struct AgentProperties {
    // TODO Of this leg of the trip only!
    pub total_time: Duration,
    pub waiting_here: Duration,
    pub total_waiting: Duration,

    // TODO More continuous on a single lane
    pub dist_crossed: Distance,
    pub total_dist: Distance,

    pub lanes_crossed: usize,
    pub total_lanes: usize,
}
