use crate::{
    AgentID, Benchmark, CarID, Command, CreateCar, DrawCarInput, DrawPedestrianInput, DrivingGoal,
    DrivingSimState, Event, GetDrawAgents, IntersectionSimState, ParkedCar, ParkingSimState,
    ParkingSpot, PedestrianID, Router, Scheduler, ScoreSummary, SimStats, Summary, TransitSimState,
    TripID, TripLeg, TripManager, TripSpawner, TripSpec, VehicleSpec, VehicleType, WalkingSimState,
    BUS_LENGTH,
};
use abstutil::Timer;
use derivative::Derivative;
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D};
use map_model::{BuildingID, BusRoute, IntersectionID, LaneID, Map, Path, Traversable};
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
    stats: Option<SimStats>,

    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    events_since_last_step: Vec<Event>,
}

// Setup
impl Sim {
    pub fn new(map: &Map, run_name: String, savestate_every: Option<Duration>) -> Sim {
        let mut scheduler = Scheduler::new();
        scheduler.push(CHECK_FOR_GRIDLOCK_FREQUENCY, Command::CheckForGridlock);
        if let Some(d) = savestate_every {
            scheduler.push(d, Command::Savestate(d));
        }
        Sim {
            driving: DrivingSimState::new(map),
            parking: ParkingSimState::new(map),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(map, &mut scheduler),
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
            run_name,
            step_count: 0,
            stats: None,
            events_since_last_step: Vec::new(),
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

    pub fn spawn_all_trips(&mut self, map: &Map, timer: &mut Timer) {
        self.spawner.spawn_all(
            map,
            &self.parking,
            &mut self.trips,
            &mut self.scheduler,
            timer,
        );
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_spots(l)
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

            // TODO Aww, we create an orphan trip if the bus can't spawn.
            let trip = self
                .trips
                .new_trip(self.time, vec![TripLeg::ServeBusRoute(id, route.id)]);
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
                timer.note(format!(
                    "Spawned bus {} for route {} ({})",
                    id, route.name, route.id
                ));
                results.push(id);
            } else {
                timer.warn(format!(
                    "No room for a bus headed towards stop {} of {} ({}), giving up",
                    next_stop_idx, route.name, route.id
                ));
            }
        }
        results
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
        // TODO Faster
        self.get_all_draw_cars(map).into_iter().find(|d| d.id == id)
    }

    fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput> {
        // TODO Faster
        self.get_all_draw_peds(map).into_iter().find(|d| d.id == id)
    }

    fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
        if let Traversable::Lane(l) = on {
            if map.get_l(l).is_parking() {
                return self.parking.get_draw_cars(l, map);
            }
        }
        self.driving.get_draw_cars_on(self.time, on, map)
    }

    fn get_draw_peds(&self, on: Traversable, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_draw_peds(self.time, on, map)
    }

    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        let mut result = self.driving.get_all_draw_cars(self.time, map);
        result.extend(self.parking.get_all_draw_cars(map));
        result
    }

    fn get_all_draw_peds(&self, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_all_draw_peds(self.time, map)
    }
}

// Drawing
impl Sim {
    // The results represent (moving, waiting) vehicles
    pub fn get_unzoomed_polygons(&self, map: &Map) -> (Vec<Polygon>, Vec<Polygon>) {
        self.driving.get_unzoomed_polygons(map)
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
                Command::SpawnCar(create_car) => {
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
                    } else {
                        self.scheduler.push(
                            self.time + BLIND_RETRY_TO_SPAWN,
                            Command::SpawnCar(create_car),
                        );
                    }
                }
                Command::SpawnPed(create_ped) => {
                    // Do the order a bit backwards so we don't have to clone the CreatePedestrian.
                    // spawn_ped can't fail.
                    self.trips.agent_starting_trip_leg(
                        AgentID::Pedestrian(create_ped.id),
                        create_ped.trip,
                    );
                    self.walking
                        .spawn_ped(self.time, create_ped, map, &mut self.scheduler);
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

        self.stats = None;

        self.events_since_last_step.clear();
        self.events_since_last_step
            .extend(self.trips.collect_events());
        self.events_since_last_step
            .extend(self.transit.collect_events());
    }

    pub fn dump_before_abort(&self) {
        println!(
            "********************************************************************************"
        );
        println!("At {}", self.time,);
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
        time_limit: Option<Duration>,
    ) {
        let mut benchmark = self.start_benchmark();
        loop {
            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                // TODO Doesn't respect time_limit!
                self.step(&map, Duration::seconds(30.0));
            })) {
                Ok(()) => {}
                Err(err) => {
                    println!("********************************************************************************");
                    println!("Sim broke:");
                    self.dump_before_abort();
                    panic::resume_unwind(err);
                }
            }

            if benchmark.has_real_time_passed(Duration::seconds(1.0)) {
                println!(
                    "{}, speed = {}",
                    self.summary(),
                    self.measure_speed(&mut benchmark, true)
                );
            }
            callback(self, map);
            if Some(self.time()) >= time_limit {
                panic!("Time limit {} hit", self.time);
            }
            if self.is_done() {
                println!(
                    "{}, speed = {}",
                    self.summary(),
                    self.measure_speed(&mut benchmark, true)
                );
                break;
            }
        }
    }

    pub fn run_until_expectations_met(
        &mut self,
        map: &Map,
        all_expectations: Vec<Event>,
        time_limit: Duration,
    ) {
        // TODO Maybe can use run_until_done for this.
        let mut benchmark = self.start_benchmark();
        let mut expectations = VecDeque::from(all_expectations);
        loop {
            if expectations.is_empty() {
                return;
            }
            // TODO Doesn't respect time_limit!
            self.step(&map, Duration::seconds(30.0));
            for ev in self.get_events_since_last_step() {
                if ev == expectations.front().unwrap() {
                    println!("At {}, met expectation {:?}", self.time, ev);
                    expectations.pop_front();
                    if expectations.is_empty() {
                        return;
                    }
                }
            }
            if benchmark.has_real_time_passed(Duration::seconds(1.0)) {
                println!(
                    "{}, speed = {}",
                    self.summary(),
                    self.measure_speed(&mut benchmark, true)
                );
            }
            if self.time() == time_limit {
                panic!(
                    "Time limit {} hit, but some expectations never met: {:?}",
                    self.time, expectations
                );
            }
        }
    }
}

// Savestating
impl Sim {
    pub fn save(&self) -> String {
        // If we wanted to be even more reproducible, we'd encode RNG seed, version of code, etc,
        // but that's overkill right now.
        let path = format!(
            "../data/save/{}_{}/{}/{}",
            self.map_name,
            self.edits_name,
            self.run_name,
            self.time.as_filename()
        );
        abstutil::write_json(&path, &self).expect("Writing sim state failed");
        println!("Saved to {}", path);
        path
    }

    pub fn find_previous_savestate(&self, base_time: Duration) -> Option<String> {
        abstutil::find_prev_file(&format!(
            "../data/save/{}_{}/{}/{}",
            self.map_name,
            self.edits_name,
            self.run_name,
            base_time.as_filename()
        ))
    }

    pub fn find_next_savestate(&self, base_time: Duration) -> Option<String> {
        abstutil::find_next_file(&format!(
            "../data/save/{}_{}/{}/{}",
            self.map_name,
            self.edits_name,
            self.run_name,
            base_time.as_filename()
        ))
    }

    pub fn load_savestate(
        path: String,
        new_run_name: Option<String>,
    ) -> Result<Sim, std::io::Error> {
        println!("Loading {}", path);
        abstutil::read_json(&path).map(|mut s: Sim| {
            if let Some(name) = new_run_name {
                s.run_name = name;
            }
            s
        })
    }
}

// Benchmarking
impl Sim {
    pub fn start_benchmark(&self) -> Benchmark {
        Benchmark {
            last_real_time: Instant::now(),
            last_sim_time: self.time,
        }
    }

    pub fn measure_speed(&self, b: &mut Benchmark, details: bool) -> String {
        let dt = Duration::seconds(abstutil::elapsed_seconds(b.last_real_time));
        if dt == Duration::ZERO {
            return "...".to_string();
        }
        let speed = (self.time - b.last_sim_time) / dt;
        b.last_real_time = Instant::now();
        b.last_sim_time = self.time;
        if details {
            format!("{:.2}x ({})", speed, self.scheduler.describe_stats())
        } else {
            format!("{:.2}x", speed)
        }
    }
}

// Queries of all sorts
impl Sim {
    pub fn time(&self) -> Duration {
        self.time
    }

    pub fn get_name(&self) -> &str {
        &self.run_name
    }

    pub fn is_done(&self) -> bool {
        self.spawner.is_done() && self.trips.is_done()
    }

    pub fn is_empty(&self) -> bool {
        self.time == Duration::ZERO && self.is_done()
    }

    // TODO Rethink this
    pub fn summarize(&self, _lanes: &HashSet<LaneID>) -> Summary {
        Summary {
            cars_parked: 0,
            open_parking_spots: 0,
            moving_cars: 0,
            stuck_cars: 0,
            buses: 0,
            moving_peds: 0,
            stuck_peds: 0,
            trips_with_ab_test_divergence: 0,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: {} active agents",
            self.time,
            abstutil::prettyprint_usize(self.trips.num_active_trips())
        )
    }

    // TODO Rethink this
    pub fn get_score(&self) -> ScoreSummary {
        ScoreSummary {
            pending_walking_trips: 0,
            total_walking_trips: 0,
            total_walking_trip_time: Duration::ZERO,
            pending_driving_trips: 0,
            total_driving_trips: 0,
            total_driving_trip_time: Duration::ZERO,
            completion_time: None,
        }
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

    pub fn ped_tooltip(&self, p: PedestrianID) -> Vec<String> {
        let mut lines = self.walking.ped_tooltip(p);
        lines.extend(self.trips.tooltip_lines(AgentID::Pedestrian(p)));
        lines
    }

    pub fn car_tooltip(&self, car: CarID) -> Vec<String> {
        if let Some(mut lines) = self.driving.tooltip_lines(car) {
            lines.extend(self.trips.tooltip_lines(AgentID::Car(car)));
            lines
        } else {
            self.parking.tooltip_lines(car).unwrap()
        }
    }

    pub fn active_agents(&self) -> Vec<AgentID> {
        self.trips.active_agents()
    }

    pub fn debug_trip(&self, id: TripID) {
        match self.trips.trip_to_agent(id) {
            Some(AgentID::Car(id)) => self.debug_car(id),
            Some(AgentID::Pedestrian(id)) => self.debug_ped(id),
            None => println!("{} doesn't exist", id),
        }
    }

    pub fn agent_to_trip(&self, id: AgentID) -> Option<TripID> {
        self.trips.agent_to_trip(id)
    }

    pub fn trip_to_agent(&self, id: TripID) -> Option<AgentID> {
        self.trips.trip_to_agent(id)
    }

    pub fn lookup_car_id(&self, idx: usize) -> Option<CarID> {
        for vt in &[VehicleType::Car, VehicleType::Bike, VehicleType::Bus] {
            let id = CarID(idx, *vt);
            if self.driving.tooltip_lines(id).is_some() {
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

    pub fn get_stats(&mut self, map: &Map) -> &SimStats {
        if self.stats.is_some() {
            return self.stats.as_ref().unwrap();
        }

        let mut stats = SimStats::new(self.time);
        for trip in self.trips.get_active_trips().into_iter() {
            if let Some(agent) = self.trips.trip_to_agent(trip) {
                stats
                    .canonical_pt_per_trip
                    .insert(trip, self.canonical_pt_for_agent(agent, map));
            }
        }

        self.stats = Some(stats);
        self.stats.as_ref().unwrap()
    }

    pub fn get_events_since_last_step(&self) -> &Vec<Event> {
        &self.events_since_last_step
    }

    pub fn get_canonical_pt_per_trip(&self, trip: TripID, map: &Map) -> Option<Pt2D> {
        self.trips
            .trip_to_agent(trip)
            .map(|id| self.canonical_pt_for_agent(id, map))
    }

    // Assumes agent does exist.
    fn canonical_pt_for_agent(&self, id: AgentID, map: &Map) -> Pt2D {
        match id {
            AgentID::Car(id) => self.get_draw_car(id, map).unwrap().body.last_pt(),
            AgentID::Pedestrian(id) => self.get_draw_ped(id, map).unwrap().pos,
        }
    }

    // TODO argh this is so inefficient
    pub fn location_for_agent(&self, id: AgentID, map: &Map) -> Traversable {
        match id {
            AgentID::Car(id) => self.get_draw_car(id, map).unwrap().on,
            AgentID::Pedestrian(id) => self.get_draw_ped(id, map).unwrap().on,
        }
    }

    pub fn get_accepted_agents(&self, id: IntersectionID) -> HashSet<AgentID> {
        self.intersections.get_accepted_agents(id)
    }

    pub fn is_in_overtime(&self, id: IntersectionID, map: &Map) -> bool {
        self.intersections.is_in_overtime(self.time, id, map)
    }
}
