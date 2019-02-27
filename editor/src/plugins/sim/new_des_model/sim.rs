use crate::plugins::sim::new_des_model::{
    Benchmark, DrivingSimState, Event, IntersectionSimState, ParkedCar, ParkingSimState,
    ParkingSpot, Scheduler, ScoreSummary, SimStats, Summary, TripManager, TripSpawner, TripSpec,
    VehicleSpec, WalkingSimState, TIMESTEP,
};
use abstutil::Timer;
use derivative::Derivative;
use ezgui::GfxCtx;
use geom::{Distance, Duration, Pt2D};
use map_model::{
    BuildingID, IntersectionID, LaneID, LaneType, Map, Path, Trace, Traversable, Turn,
};
use serde_derive::{Deserialize, Serialize};
use sim::{
    AgentID, CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, TripID,
    VehicleType,
};
use std::collections::{HashSet, VecDeque};
use std::panic;
use std::time::Instant;

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    trips: TripManager,
    scheduler: Scheduler,
    spawner: TripSpawner,
    time: Duration,

    // TODO Reconsider these
    pub(crate) map_name: String,
    pub(crate) edits_name: String,
    // Some tests deliberately set different scenario names for comparisons.
    #[derivative(PartialEq = "ignore")]
    run_name: String,
    #[derivative(PartialEq = "ignore")]
    savestate_every: Option<Duration>,

    // Lazily computed.
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    stats: Option<SimStats>,
}

// Setup
impl Sim {
    pub fn new(map: &Map, run_name: String, savestate_every: Option<Duration>) -> Sim {
        Sim {
            driving: DrivingSimState::new(map),
            parking: ParkingSimState::new(map),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(map),
            trips: TripManager::new(),
            scheduler: Scheduler::new(),
            spawner: TripSpawner::new(),
            time: Duration::ZERO,

            map_name: map.get_name().to_string(),
            edits_name: map.get_edits().edits_name.to_string(),
            run_name,
            savestate_every,
            stats: None,
        }
    }

    pub fn schedule_trip(&mut self, start_time: Duration, spec: TripSpec, map: &Map) {
        self.spawner
            .schedule_trip(start_time, spec, map, &self.parking);
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
    ) {
        self.parking.reserve_spot(spot);
        self.parking.add_parked_car(ParkedCar {
            vehicle: vehicle.make(
                CarID::tmp_new(self.spawner.car_id_counter, VehicleType::Car),
                owner,
            ),
            spot,
        });
        self.spawner.car_id_counter += 1;
    }

    pub fn get_parked_cars_by_owner(&self, bldg: BuildingID) -> Vec<&ParkedCar> {
        self.parking.get_parked_cars_by_owner(bldg)
    }
}

// Drawing
impl GetDrawAgents for Sim {
    fn time(&self) -> Duration {
        self.time
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
    pub fn draw_unzoomed(&self, g: &mut GfxCtx, map: &Map) {
        self.driving.draw_unzoomed(self.time, g, map);
    }
}

// Running
impl Sim {
    pub fn step(&mut self, map: &Map) -> Vec<Event> {
        self.time += TIMESTEP;

        self.driving.step_if_needed(
            self.time,
            map,
            &mut self.parking,
            &mut self.intersections,
            &mut self.trips,
            &mut self.scheduler,
        );
        self.walking.step_if_needed(
            self.time,
            map,
            &mut self.intersections,
            &self.parking,
            &mut self.scheduler,
            &mut self.trips,
        );

        // Spawn stuff at the end, so we can see the correct state of everything else at this time.
        self.scheduler.step_if_needed(
            self.time,
            map,
            &mut self.parking,
            &mut self.walking,
            &mut self.driving,
            &self.intersections,
            &mut self.trips,
        );

        self.stats = None;

        // Savestate? Do this AFTER incrementing the timestep. Otherwise we could repeatedly load a
        // savestate, run a step, and invalidly save over it.
        if let Some(t) = self.savestate_every {
            if self.time.is_multiple_of(t) {
                self.save();
            }
        }

        Vec::new()
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
    pub fn run_until_done<F: Fn(&Sim)>(
        &mut self,
        map: &Map,
        callback: F,
        time_limit: Option<Duration>,
    ) {
        let mut benchmark = self.start_benchmark();
        loop {
            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.step(&map);
            })) {
                Ok(()) => {}
                Err(err) => {
                    println!("********************************************************************************");
                    println!("Sim broke:");
                    self.dump_before_abort();
                    panic::resume_unwind(err);
                }
            }

            if self.time().is_multiple_of(Duration::minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                println!("{0}, speed = {1:.2}x", self.summary(), speed);
            }
            callback(self);
            if Some(self.time()) == time_limit {
                panic!("Time limit {} hit", self.time);
            }
            if self.is_done() {
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
        let mut benchmark = self.start_benchmark();
        let mut expectations = VecDeque::from(all_expectations);
        loop {
            if expectations.is_empty() {
                return;
            }
            for ev in self.step(&map).into_iter() {
                if ev == *expectations.front().unwrap() {
                    println!("At {}, met expectation {:?}", self.time, ev);
                    expectations.pop_front();
                    if expectations.is_empty() {
                        return;
                    }
                }
            }
            if self.time().is_multiple_of(Duration::minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                println!("{0}, speed = {1:.2}x", self.summary(), speed);
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

    pub fn measure_speed(&self, b: &mut Benchmark) -> f64 {
        let dt = Duration::seconds(abstutil::elapsed_seconds(b.last_real_time));
        let speed = (self.time - b.last_sim_time) / dt;
        b.last_real_time = Instant::now();
        b.last_sim_time = self.time;
        speed
    }
}

// Live modification -- TODO rethink all of this
impl Sim {
    pub fn edit_lane_type(&mut self, _id: LaneID, _old_type: LaneType, _map: &Map) {
        panic!("implement");
    }

    pub fn edit_remove_turn(&mut self, _t: &Turn) {
        panic!("implement");
    }

    pub fn edit_add_turn(&mut self, _t: &Turn) {
        panic!("implement");
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
            "{}, {} active agents",
            self.time,
            self.trips.active_agents().len()
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

    pub fn debug_intersection(&mut self, id: IntersectionID, map: &Map) {
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
            let id = CarID::tmp_new(idx, *vt);
            if self.driving.tooltip_lines(id).is_some() {
                return Some(id);
            }
        }

        let id = CarID::tmp_new(idx, VehicleType::Car);
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
    ) -> Option<Trace> {
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
