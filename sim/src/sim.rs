use crate::{
    AgentID, Analytics, CarID, Command, CreateCar, DrawCarInput, DrawPedCrowdInput,
    DrawPedestrianInput, DrivingGoal, DrivingSimState, Event, GetDrawAgents, IntersectionSimState,
    ParkedCar, ParkingSimState, ParkingSpot, PedestrianID, Person, PersonID, PersonState, Router,
    Scheduler, SidewalkPOI, SidewalkSpot, TransitSimState, TripCount, TripEnd, TripID, TripLeg,
    TripManager, TripMode, TripPhaseType, TripPositions, TripResult, TripSpawner, TripSpec,
    TripStart, UnzoomedAgent, VehicleSpec, VehicleType, WalkingSimState, BUS_LENGTH,
};
use abstutil::Timer;
use derivative::Derivative;
use geom::{Distance, Duration, PolyLine, Pt2D, Time};
use instant::Instant;
use map_model::{
    BuildingID, BusRoute, BusRouteID, IntersectionID, LaneID, Map, Path, PathConstraints,
    PathRequest, PathStep, RoadID, Traversable,
};
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
    scheduler: Scheduler,
    time: Time,
    car_id_counter: usize,
    ped_id_counter: usize,

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
}

#[derive(Clone)]
pub struct SimOptions {
    pub run_name: String,
    pub savestate_every: Option<Duration>,
    pub use_freeform_policy_everywhere: bool,
    pub disable_block_the_box: bool,
    pub recalc_lanechanging: bool,
    pub clear_laggy_head_early: bool,
}

impl SimOptions {
    pub fn new(run_name: &str) -> SimOptions {
        SimOptions {
            run_name: run_name.to_string(),
            savestate_every: None,
            use_freeform_policy_everywhere: false,
            disable_block_the_box: false,
            recalc_lanechanging: true,
            clear_laggy_head_early: false,
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
            driving: DrivingSimState::new(
                map,
                opts.recalc_lanechanging,
                opts.clear_laggy_head_early,
            ),
            parking: ParkingSimState::new(map, timer),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(
                map,
                &mut scheduler,
                opts.use_freeform_policy_everywhere,
                opts.disable_block_the_box,
            ),
            transit: TransitSimState::new(),
            trips: TripManager::new(),
            scheduler,
            time: Time::START_OF_DAY,
            car_id_counter: 0,
            ped_id_counter: 0,

            map_name: map.get_name().to_string(),
            // TODO
            edits_name: "untitled edits".to_string(),
            run_name: opts.run_name,
            step_count: 0,
            trip_positions: None,
            check_for_gridlock: None,

            analytics: Analytics::new(),
        }
    }

    pub fn make_spawner(&self) -> TripSpawner {
        TripSpawner::new()
    }
    pub fn flush_spawner(
        &mut self,
        spawner: TripSpawner,
        map: &Map,
        timer: &mut Timer,
        retry_if_no_room: bool,
    ) {
        spawner.finalize(
            map,
            &mut self.trips,
            &mut self.scheduler,
            &self.parking,
            timer,
            retry_if_no_room,
        );
    }
    // TODO Friend method pattern :(
    pub(crate) fn spawner_parking(&self) -> &ParkingSimState {
        &self.parking
    }
    pub(crate) fn spawner_new_car_id(&mut self) -> usize {
        let id = self.car_id_counter;
        self.car_id_counter += 1;
        id
    }
    pub(crate) fn spawner_new_ped_id(&mut self) -> usize {
        let id = self.ped_id_counter;
        self.ped_id_counter += 1;
        id
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_spots(l)
    }

    pub fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot> {
        self.parking.get_free_offstreet_spots(b)
    }

    // (Filled, available)
    pub fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        self.parking.get_all_parking_spots()
    }

    // TODO Should these two be in TripSpawner?
    pub fn new_person(&mut self, p: PersonID) {
        self.trips.new_person(p);
    }
    pub fn random_person(&mut self) -> PersonID {
        self.trips.random_person()
    }
    pub fn seed_parked_car(
        &mut self,
        vehicle: VehicleSpec,
        spot: ParkingSpot,
        owner: Option<BuildingID>,
    ) -> CarID {
        let id = CarID(self.car_id_counter, VehicleType::Car);
        self.car_id_counter += 1;

        self.parking.reserve_spot(spot);
        self.parking.add_parked_car(ParkedCar {
            vehicle: vehicle.make(id, owner),
            spot,
        });
        id
    }

    pub fn get_parked_cars_by_owner(&self, bldg: BuildingID) -> Vec<&ParkedCar> {
        self.parking.get_parked_cars_by_owner(bldg)
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
            let id = CarID(self.car_id_counter, VehicleType::Bus);
            self.car_id_counter += 1;

            // For now, no desire for randomness. Caller can pass in list of specs if that ever
            // changes.
            let vehicle = VehicleSpec {
                vehicle_type: VehicleType::Bus,
                length: BUS_LENGTH,
                max_speed: None,
            }
            .make(id, None);

            // TODO The path analytics (total dist, dist crossed so far) will be wrong for the
            // first round of buses.
            // Same for this TripStart, though it doesn't matter too much.
            let trip = self.trips.new_trip(
                None,
                self.time,
                TripStart::Border(map.get_l(path.current_step().as_lane()).src_i),
                vec![TripLeg::ServeBusRoute(id, route.id)],
            );

            loop {
                if path.is_last_step() {
                    timer.warn(format!(
                        "Giving up on seeding a bus headed towards stop {} of {} ({})",
                        next_stop_idx, route.name, route.id
                    ));
                    self.trips.abort_trip_failed_start(trip);
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
                        trip,
                    },
                    map,
                    &self.intersections,
                    &self.parking,
                    &mut self.scheduler,
                ) {
                    self.trips.agent_starting_trip_leg(AgentID::Car(id), trip);
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
            Command::SpawnCar(create_car, retry_if_no_room) => {
                if self.driving.start_car_on_lane(
                    self.time,
                    create_car.clone(),
                    map,
                    &self.intersections,
                    &self.parking,
                    &mut self.scheduler,
                ) {
                    self.trips.agent_starting_trip_leg(
                        AgentID::Car(create_car.vehicle.id),
                        create_car.trip,
                    );
                    if let Some(parked_car) = create_car.maybe_parked_car {
                        self.parking.remove_parked_car(parked_car);
                    }
                    events.push(Event::TripPhaseStarting(
                        create_car.trip,
                        // TODO sketchy...
                        if create_car.vehicle.id.1 == VehicleType::Car {
                            TripMode::Drive
                        } else {
                            TripMode::Bike
                        },
                        Some(create_car.req.clone()),
                        if create_car.vehicle.id.1 == VehicleType::Car {
                            TripPhaseType::Driving
                        } else {
                            TripPhaseType::Biking
                        },
                    ));
                    self.analytics
                        .record_demand(create_car.router.get_path(), map);
                } else if retry_if_no_room {
                    // TODO Record this in the trip log
                    self.scheduler.push(
                        self.time + BLIND_RETRY_TO_SPAWN,
                        Command::SpawnCar(create_car, retry_if_no_room),
                    );
                } else {
                    println!(
                        "No room to spawn car for {}. Not retrying!",
                        create_car.trip
                    );
                    self.trips.abort_trip_failed_start(create_car.trip);
                }
            }
            Command::SpawnPed(mut create_ped) => {
                let ok = if let SidewalkPOI::DeferredParkingSpot(b, driving_goal) =
                    create_ped.goal.connection.clone()
                {
                    if let Some(parked_car) = self.parking.dynamically_reserve_car(b) {
                        create_ped.goal =
                            SidewalkSpot::parking_spot(parked_car.spot, map, &self.parking);
                        create_ped.req = PathRequest {
                            start: create_ped.start.sidewalk_pos,
                            end: create_ped.goal.sidewalk_pos,
                            constraints: PathConstraints::Pedestrian,
                        };
                        if let Some(path) = map.pathfind(create_ped.req.clone()) {
                            create_ped.path = path;
                            let mut legs = vec![
                                TripLeg::Walk(
                                    create_ped.id,
                                    create_ped.speed,
                                    create_ped.goal.clone(),
                                ),
                                TripLeg::Drive(parked_car.vehicle.clone(), driving_goal.clone()),
                            ];
                            match driving_goal {
                                DrivingGoal::ParkNear(b) => {
                                    legs.push(TripLeg::Walk(
                                        create_ped.id,
                                        create_ped.speed,
                                        SidewalkSpot::building(b, map),
                                    ));
                                }
                                DrivingGoal::Border(_, _) => {}
                            }
                            self.trips.dynamically_override_legs(create_ped.trip, legs);
                            true
                        } else {
                            println!(
                                "WARNING: At {}, {} giving up because no path from {} to {:?}",
                                self.time, create_ped.id, b, create_ped.goal.connection
                            );
                            self.parking.dynamically_return_car(parked_car);
                            false
                        }
                    } else {
                        println!(
                            "WARNING: At {}, no free car for {} spawning at {}",
                            self.time, create_ped.id, b
                        );
                        false
                    }
                } else {
                    true
                };
                if ok {
                    // Do the order a bit backwards so we don't have to clone the
                    // CreatePedestrian. spawn_ped can't fail.
                    self.trips.agent_starting_trip_leg(
                        AgentID::Pedestrian(create_ped.id),
                        create_ped.trip,
                    );
                    events.push(Event::TripPhaseStarting(
                        create_ped.trip,
                        TripMode::Walk,
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
                                map,
                                &self.parking,
                                &mut self.scheduler,
                            );
                        }
                        _ => {
                            self.walking
                                .spawn_ped(self.time, create_ped, map, &mut self.scheduler);
                        }
                    }
                } else {
                    self.trips.abort_trip_failed_start(create_ped.trip);
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
                    &self.parking,
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
        }

        // Record events at precisely the time they occur.
        events.extend(self.trips.collect_events());
        events.extend(self.transit.collect_events());
        events.extend(self.driving.collect_events());
        events.extend(self.walking.collect_events());
        events.extend(self.intersections.collect_events());
        for ev in events {
            self.analytics.event(ev, self.time, map);
        }

        savestate
    }

    pub fn timed_step(&mut self, map: &Map, dt: Duration, timer: &mut Timer) {
        let end_time = self.time + dt;
        let start = Instant::now();
        let mut last_update = Instant::now();

        timer.start(format!("Advance sim to {}", end_time));
        while self.time < end_time {
            self.minimal_step(map, end_time - self.time);
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
                let (finished, unfinished, _, _, _) = self.num_trips();
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

    // (number of finished trips, number of unfinished trips, number of active by mode, number of
    // people in buildings, number of people off map)
    pub fn num_trips(&self) -> (usize, usize, BTreeMap<TripMode, usize>, usize, usize) {
        self.trips.num_trips()
    }

    pub fn count_trips_involving_bldg(&self, b: BuildingID) -> TripCount {
        self.trips.count_trips_involving_bldg(b, self.time)
    }
    pub fn count_trips_involving_border(&self, i: IntersectionID) -> TripCount {
        self.trips.count_trips_involving_border(i, self.time)
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

    pub fn ped_properties(
        &self,
        p: PedestrianID,
        map: &Map,
    ) -> (Vec<(String, String)>, Vec<String>) {
        self.walking.ped_properties(p, self.time, map)
    }

    pub fn car_properties(&self, car: CarID, map: &Map) -> (Vec<(String, String)>, Vec<String>) {
        if let Some((mut props, extra)) = self.driving.car_properties(car, self.time, map) {
            if car.1 == VehicleType::Bus {
                props.push((
                    "Route".to_string(),
                    map.get_br(self.transit.bus_route(car)).name.clone(),
                ));
                let passengers = self.transit.get_passengers(car);
                props.push(("Passengers".to_string(), passengers.len().to_string()));
                // TODO Clean this up
                /*for (id, stop) in passengers {
                    extra.push(format!("- {} till {:?}", id, stop));
                }*/
            }
            (props, extra)
        } else {
            let mut props = Vec::new();
            let mut extra = Vec::new();
            if let Some(b) = self.parking.get_owner_of_car(car) {
                props.push(("Owner".to_string(), map.get_b(b).just_address(map)));
                // TODO More details here
                if let Some((trip, start)) = self.trips.find_trip_using_car(car, b) {
                    extra.push(format!(
                        "{} will use this car, sometime after {}",
                        trip, start
                    ));
                }
            } else {
                props.push((
                    "Owner".to_string(),
                    "visiting from outside the map".to_string(),
                ));
            }
            (props, extra)
        }
    }

    // Percent in [0, 1]
    // TODO More continuous on a single lane
    pub fn progress_along_path(&self, agent: AgentID) -> Option<f64> {
        match agent {
            AgentID::Car(c) => {
                if c.1 != VehicleType::Bus {
                    self.driving.progress_along_path(c)
                } else {
                    None
                }
            }
            AgentID::Pedestrian(p) => self.walking.progress_along_path(p),
        }
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

    pub fn trip_endpoints(&self, id: TripID) -> (TripStart, TripEnd) {
        self.trips.trip_endpoints(id)
    }

    pub fn trip_to_person(&self, id: TripID) -> Option<PersonID> {
        self.trips.trip_to_person(id)
    }

    pub fn get_person(&self, id: PersonID) -> &Person {
        self.trips.get_person(id)
    }
    pub fn get_all_people(&self) -> &Vec<Person> {
        self.trips.get_all_people()
    }

    pub fn trip_start_time(&self, id: TripID) -> Time {
        self.trips.trip_start_time(id)
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
        if self.parking.does_car_exist(id) {
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

    pub fn get_owner_of_car(&self, id: CarID) -> Option<BuildingID> {
        self.driving
            .get_owner_of_car(id)
            .or_else(|| self.parking.get_owner_of_car(id))
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
        match self.trips.get_person(p).state {
            PersonState::Inside(b) => Some(map.get_b(b).polygon.center()),
            PersonState::Trip(t) => self.get_canonical_pt_per_trip(t, map).ok(),
            PersonState::OffMap | PersonState::Limbo => None,
        }
    }

    pub fn does_agent_exist(&self, id: AgentID) -> bool {
        match id {
            AgentID::Car(id) => self.parking.does_car_exist(id) || self.driving.does_car_exist(id),
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

    pub fn trip_spec_to_path_req(&self, spec: &TripSpec, map: &Map) -> PathRequest {
        spec.get_pathfinding_request(map, &self.parking)
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
}

// Invasive debugging
impl Sim {
    pub fn kill_stuck_car(&mut self, id: CarID, map: &Map) {
        if let Some(trip) = self.agent_to_trip(AgentID::Car(id)) {
            self.trips.abort_trip_failed_start(trip);
            self.driving.kill_stuck_car(
                id,
                self.time,
                map,
                &mut self.scheduler,
                &mut self.intersections,
            );
            println!("Forcibly killed {}", id);
        } else {
            println!("{} has no trip?!", id);
        }
    }
}
