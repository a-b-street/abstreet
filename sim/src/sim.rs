use crate::{
    AgentID, CarID, Command, CreateCar, DrawCarInput, DrawPedCrowdInput, DrawPedestrianInput,
    DrivingGoal, DrivingSimState, Event, FinishedTrips, GetDrawAgents, IntersectionSimState,
    ParkedCar, ParkingSimState, ParkingSpot, PedestrianID, Router, Scheduler, SidewalkPOI,
    SidewalkSpot, TransitSimState, TripID, TripLeg, TripManager, TripPositions, TripResult,
    TripSpawner, TripSpec, TripStart, TripStatus, UnzoomedAgent, VehicleSpec, VehicleType,
    WalkingSimState, BUS_LENGTH,
};
use abstutil::{elapsed_seconds, Timer};
use derivative::Derivative;
use geom::{Distance, Duration, DurationHistogram, PolyLine, Pt2D};
use map_model::{
    BuildingID, BusRoute, BusRouteID, IntersectionID, LaneID, Map, Path, PathRequest, Position,
    Traversable,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::panic;
use std::time::Instant;

const CHECK_FOR_GRIDLOCK_FREQUENCY: Duration = Duration::const_seconds(5.0 * 60.0);
// TODO Do something else.
const BLIND_RETRY_TO_SPAWN: Duration = Duration::const_seconds(5.0);

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    transit: TransitSimState,
    trips: TripManager,
    spawner: TripSpawner,
    scheduler: Scheduler,
    time: Duration,
    car_id_counter: usize,
    ped_id_counter: usize,

    // TODO Reconsider these
    pub(crate) map_name: String,
    pub(crate) edits_name: String,
    // Some tests deliberately set different scenario names for comparisons.
    #[derivative(PartialEq = "ignore")]
    run_name: String,
    #[derivative(PartialEq = "ignore")]
    step_count: usize,

    // Lazily computed.
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    trip_positions: Option<TripPositions>,
    // TODO Maybe the buffered events in child objects should also have this.
}

#[derive(Clone)]
pub struct SimOptions {
    pub run_name: String,
    pub savestate_every: Option<Duration>,
    pub use_freeform_policy_everywhere: bool,
    pub disable_block_the_box: bool,
    pub record_stats: bool,
    pub recalc_lanechanging: bool,
}

impl SimOptions {
    pub fn new(run_name: &str) -> SimOptions {
        SimOptions {
            run_name: run_name.to_string(),
            savestate_every: None,
            use_freeform_policy_everywhere: false,
            disable_block_the_box: false,
            record_stats: false,
            recalc_lanechanging: true,
        }
    }
}

// Setup
impl Sim {
    pub fn new(map: &Map, opts: SimOptions) -> Sim {
        let mut scheduler = Scheduler::new();
        // TODO Gridlock detection doesn't add value right now.
        if false {
            scheduler.push(CHECK_FOR_GRIDLOCK_FREQUENCY, Command::CheckForGridlock);
        }
        if let Some(d) = opts.savestate_every {
            scheduler.push(d, Command::Savestate(d));
        }
        Sim {
            driving: DrivingSimState::new(map, opts.recalc_lanechanging),
            parking: ParkingSimState::new(map),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(
                map,
                &mut scheduler,
                opts.use_freeform_policy_everywhere,
                opts.disable_block_the_box,
            ),
            transit: TransitSimState::new(),
            trips: TripManager::new(),
            spawner: TripSpawner::new(),
            scheduler,
            time: Duration::ZERO,
            car_id_counter: 0,
            ped_id_counter: 0,

            map_name: map.get_name().to_string(),
            // TODO
            edits_name: "no_edits".to_string(),
            run_name: opts.run_name,
            step_count: 0,
            trip_positions: None,
        }
    }

    pub fn schedule_trip(
        &mut self,
        start_time: Duration,
        spec: TripSpec,
        map: &Map,
    ) -> (Option<PedestrianID>, Option<CarID>) {
        let (ped_id, car_id) = match spec {
            TripSpec::CarAppearing {
                ref vehicle_spec,
                ref goal,
                ..
            } => {
                let car = CarID(self.car_id_counter, vehicle_spec.vehicle_type);
                self.car_id_counter += 1;
                let ped = match goal {
                    DrivingGoal::ParkNear(_) => {
                        let id = PedestrianID(self.ped_id_counter);
                        self.ped_id_counter += 1;
                        Some(id)
                    }
                    _ => None,
                };
                (ped, Some(car))
            }
            TripSpec::UsingParkedCar { .. }
            | TripSpec::MaybeUsingParkedCar { .. }
            | TripSpec::JustWalking { .. }
            | TripSpec::UsingTransit { .. } => {
                let id = PedestrianID(self.ped_id_counter);
                self.ped_id_counter += 1;
                (Some(id), None)
            }
            TripSpec::UsingBike { .. } => {
                let ped = PedestrianID(self.ped_id_counter);
                self.ped_id_counter += 1;
                let car = CarID(self.car_id_counter, VehicleType::Bike);
                self.car_id_counter += 1;
                (Some(ped), Some(car))
            }
        };

        self.spawner
            .schedule_trip(start_time, ped_id, car_id, spec, map, &self.parking);
        (ped_id, car_id)
    }

    pub fn spawn_all_trips(&mut self, map: &Map, timer: &mut Timer, retry_if_no_room: bool) {
        self.spawner.spawn_all(
            map,
            &self.parking,
            &mut self.trips,
            &mut self.scheduler,
            timer,
            retry_if_no_room,
        );
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_spots(l)
    }

    // (Filled, available)
    pub fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        self.parking.get_all_parking_spots()
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

        // Try to spawn a bus at each stop
        for (next_stop_idx, start_dist, path, end_dist) in
            self.transit.create_empty_route(route, map).into_iter()
        {
            // For now, no desire for randomness. Caller can pass in list of specs if that ever
            // changes.
            let vehicle_spec = VehicleSpec {
                vehicle_type: VehicleType::Bus,
                length: BUS_LENGTH,
                max_speed: None,
            };

            // TODO Do this validation more up-front in the map layer
            if start_dist < vehicle_spec.length {
                timer.warn(format!(
                    "Stop at {:?} is too short to spawn a bus there; giving up on one bus for {}",
                    path.current_step(),
                    route.id
                ));
                continue;
            }

            let id = CarID(self.car_id_counter, VehicleType::Bus);
            self.car_id_counter += 1;

            // Bypass some layers of abstraction that don't make sense for buses.

            let trip = self.trips.new_trip(
                self.time,
                TripStart::Appearing(Position::new(path.current_step().as_lane(), start_dist)),
                vec![TripLeg::ServeBusRoute(id, route.id)],
            );
            if self.driving.start_car_on_lane(
                self.time,
                CreateCar {
                    vehicle: vehicle_spec.make(id, None),
                    router: Router::follow_bus_route(path, end_dist),
                    start_dist,
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
                results.push(id);
            } else {
                timer.warn(format!(
                    "No room for a bus headed towards stop {} of {} ({}), giving up",
                    next_stop_idx, route.name, route.id
                ));
                self.trips.abort_trip_failed_start(trip);
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
    fn time(&self) -> Duration {
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
    pub fn step(&mut self, map: &Map, dt: Duration) {
        self.step_count += 1;
        if !self.spawner.is_done() {
            panic!("Forgot to call spawn_all_trips");
        }

        let target_time = self.time + dt;
        let mut savestate_at: Option<Duration> = None;
        while let Some((cmd, time)) = self.scheduler.get_next(target_time) {
            // Many commands might be scheduled for a particular time. Savestate at the END of a
            // certain time.
            if let Some(t) = savestate_at {
                if time > t {
                    self.time = t;
                    self.save();
                    savestate_at = None;
                }
            }

            self.time = time;
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
                    } else if retry_if_no_room {
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
                            if let Some(path) = map.pathfind(PathRequest {
                                start: create_ped.start.sidewalk_pos,
                                end: create_ped.goal.sidewalk_pos,
                                can_use_bike_lanes: false,
                                can_use_bus_lanes: false,
                            }) {
                                create_ped.path = path;
                                let mut legs = vec![
                                    TripLeg::Walk(
                                        create_ped.id,
                                        create_ped.speed,
                                        create_ped.goal.clone(),
                                    ),
                                    TripLeg::Drive(
                                        parked_car.vehicle.clone(),
                                        driving_goal.clone(),
                                    ),
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
                        // CreatePedestrian.  spawn_ped can't fail.
                        self.trips.agent_starting_trip_leg(
                            AgentID::Pedestrian(create_ped.id),
                            create_ped.trip,
                        );
                        self.walking
                            .spawn_ped(self.time, create_ped, map, &mut self.scheduler);
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
                Command::CheckForGridlock => {
                    if self.driving.detect_gridlock(map) {
                        self.save();
                    } else {
                        self.scheduler.push(
                            self.time + CHECK_FOR_GRIDLOCK_FREQUENCY,
                            Command::CheckForGridlock,
                        );
                    }
                }
                Command::Savestate(frequency) => {
                    self.scheduler
                        .push(self.time + frequency, Command::Savestate(frequency));
                    assert_eq!(savestate_at, None);
                    savestate_at = Some(self.time);
                }
            }
        }
        if let Some(t) = savestate_at {
            self.time = t;
            self.save();
        }
        self.time = target_time;

        self.trip_positions = None;
    }

    pub fn timed_step(&mut self, map: &Map, dt: Duration, timer: &mut Timer) {
        // TODO Ideally print every second or so
        let orig_time = self.time;
        let chunks = (dt / Duration::seconds(10.0)).ceil() as usize;
        timer.start_iter(&format!("advance simulation by {}", dt), chunks);
        for i in 0..chunks {
            timer.next();
            self.step(
                map,
                if i == chunks - 1 {
                    orig_time + dt - self.time
                } else {
                    dt * (1.0 / (chunks as f64))
                },
            );
        }
        assert_eq!(self.time, orig_time + dt);
    }

    pub fn time_limited_step(&mut self, map: &Map, dt: Duration, real_time_limit: Duration) {
        let started_at = Instant::now();
        let goal_time = self.time + dt;

        loop {
            if Duration::seconds(elapsed_seconds(started_at)) > real_time_limit
                || self.time >= goal_time
            {
                break;
            }
            self.step(map, Duration::seconds(0.1));
        }
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
impl Sim {
    pub fn just_run_until_done(&mut self, map: &Map, time_limit: Option<Duration>) {
        self.run_until_done(map, |_, _| {}, time_limit);
    }

    pub fn run_until_done<F: Fn(&Sim, &Map)>(
        &mut self,
        map: &Map,
        callback: F,
        // Interpreted as a relative time
        time_limit: Option<Duration>,
    ) {
        let mut last_print = Instant::now();
        let mut last_sim_time = self.time();

        loop {
            let dt = if let Some(lim) = time_limit {
                // TODO Regular printing then doesn't happen :\
                self.time() + lim
            } else {
                Duration::seconds(30.0)
            };

            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.step(&map, dt);
            })) {
                Ok(()) => {}
                Err(err) => {
                    println!("********************************************************************************");
                    println!("Sim broke:");
                    self.dump_before_abort();
                    panic::resume_unwind(err);
                }
            }

            let dt_real = Duration::seconds(elapsed_seconds(last_print));
            if dt_real >= Duration::seconds(1.0) {
                let (active, unfinished) = self.num_trips();
                println!(
                    "{}: {} active / {} unfinished, speed = {:.2}x, {}",
                    self.time(),
                    active,
                    unfinished,
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

        let mut expectations = VecDeque::from(all_expectations);
        self.step(&map, self.time() + time_limit);
        for ev in self.collect_events() {
            if &ev == expectations.front().unwrap() {
                println!("At {}, met expectation {:?}", self.time, ev);
                expectations.pop_front();
                if expectations.is_empty() {
                    return;
                }
            }
        }
        if expectations.is_empty() {
            return;
        }
        panic!(
            "Time limit {} hit, but some expectations never met: {:?}",
            time_limit, expectations
        );
    }
}

// Savestating
impl Sim {
    pub fn save_dir(&self) -> String {
        abstutil::path2_dir(
            &self.map_name,
            abstutil::SAVE,
            &format!("{}_{}", self.edits_name, self.run_name),
        )
    }

    fn save_path(&self, base_time: Duration) -> String {
        // If we wanted to be even more reproducible, we'd encode RNG seed, version of code, etc,
        // but that's overkill right now.
        abstutil::path2_bin(
            &self.map_name,
            abstutil::SAVE,
            &format!("{}_{}", self.edits_name, self.run_name),
            &base_time.as_filename(),
        )
    }

    pub fn save(&self) -> String {
        let path = self.save_path(self.time);
        abstutil::write_binary(&path, &self).expect("Writing sim state failed");
        println!("Saved to {}", path);
        path
    }

    pub fn find_previous_savestate(&self, base_time: Duration) -> Option<String> {
        abstutil::find_prev_file(self.save_path(base_time))
    }

    pub fn find_next_savestate(&self, base_time: Duration) -> Option<String> {
        abstutil::find_next_file(self.save_path(base_time))
    }

    pub fn load_savestate(path: String, timer: &mut Timer) -> Result<Sim, std::io::Error> {
        println!("Loading {}", path);
        abstutil::read_binary(&path, timer)
    }
}

// Queries of all sorts
impl Sim {
    pub fn time(&self) -> Duration {
        self.time
    }

    pub fn is_done(&self) -> bool {
        self.spawner.is_done() && self.trips.is_done()
    }

    pub fn is_empty(&self) -> bool {
        self.time == Duration::ZERO && self.is_done()
    }

    // (active, unfinished) prettyprinted
    pub fn num_trips(&self) -> (String, String) {
        let (active, unfinished) = self.trips.num_trips();
        (
            abstutil::prettyprint_usize(active),
            abstutil::prettyprint_usize(unfinished),
        )
    }

    pub fn get_finished_trips(&self) -> FinishedTrips {
        self.trips.get_finished_trips()
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        self.walking.debug_ped(id);
    }

    pub fn debug_car(&self, id: CarID) {
        self.driving.debug_car(id);
    }

    pub fn debug_intersection(&self, id: IntersectionID, map: &Map) {
        self.intersections.debug(id, map);
    }

    pub fn debug_lane(&self, id: LaneID) {
        self.driving.debug_lane(id);
    }

    pub fn ped_tooltip(&self, p: PedestrianID, map: &Map) -> Vec<String> {
        let mut lines = self.walking.ped_tooltip(p, self.time, map);
        lines.extend(self.trips.tooltip_lines(AgentID::Pedestrian(p)));
        lines
    }

    pub fn car_tooltip(&self, car: CarID) -> Vec<String> {
        if let Some(mut lines) = self.driving.tooltip_lines(car, self.time) {
            lines.extend(self.trips.tooltip_lines(AgentID::Car(car)));
            if car.1 == VehicleType::Bus {
                let passengers = self.transit.get_passengers(car);
                lines.push(format!("{} passengers riding", passengers.len()));
                for (id, stop) in passengers {
                    lines.push(format!("- {} till {:?}", id, stop));
                }
            }
            lines
        } else {
            let mut lines = self.parking.tooltip_lines(car).unwrap();
            if let Some(b) = self.parking.get_owner_of_car(car) {
                if let Some((trip, start)) = self.trips.find_trip_using_car(car, b) {
                    lines.push(format!(
                        "{} will use this car, sometime after {}",
                        trip, start
                    ));
                } else {
                    lines.push("Though this car has owner, no trips using it".to_string());
                }
            }
            lines
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

    pub fn trip_status(&self, id: TripID) -> TripStatus {
        self.trips.trip_status(id)
    }

    pub fn lookup_car_id(&self, idx: usize) -> Option<CarID> {
        for vt in &[VehicleType::Car, VehicleType::Bike, VehicleType::Bus] {
            let id = CarID(idx, *vt);
            if self.driving.tooltip_lines(id, self.time).is_some() {
                return Some(id);
            }
        }

        let id = CarID(idx, VehicleType::Car);
        // Only cars can be parked.
        if self.parking.tooltip_lines(id).is_some() {
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

    // This only supports one caller! And the result isn't time-sorted.
    // TODO If nobody calls this, slow sad memory leak. Push style would probably be much nicer.
    pub fn collect_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        events.extend(self.trips.collect_events());
        events.extend(self.transit.collect_events());
        events.extend(self.driving.collect_events());
        events.extend(self.walking.collect_events());
        events
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

    pub fn get_intersection_delays(&self, id: IntersectionID) -> &DurationHistogram {
        self.intersections.get_intersection_delays(id)
    }

    pub fn location_of_buses(&self, route: BusRouteID, map: &Map) -> Vec<(CarID, Pt2D)> {
        let mut results = Vec::new();
        for car in self.transit.buses_for_route(route) {
            // TODO This is a slow, indirect method!
            results.push((
                car,
                self.canonical_pt_for_agent(AgentID::Car(car), map).unwrap(),
            ));
        }
        results
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
