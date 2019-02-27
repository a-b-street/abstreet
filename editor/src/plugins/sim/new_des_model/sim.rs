use crate::plugins::sim::new_des_model::{
    Benchmark, DrivingSimState, Event, IntersectionSimState, ParkedCar, ParkingSimState,
    ParkingSpot, Scheduler, ScoreSummary, Summary, TripManager, TripSpawner, TripSpec, VehicleSpec,
    WalkingSimState,
};
use abstutil::Timer;
use ezgui::GfxCtx;
use geom::Duration;
use map_model::{BuildingID, LaneID, Map, Traversable};
use serde_derive::{Deserialize, Serialize};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, VehicleType};
use std::collections::{HashSet, VecDeque};
use std::panic;
use std::time::Instant;

#[derive(Serialize, Deserialize)]
//#[derivative(PartialEq)]
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
    //#[derivative(PartialEq = "ignore")]
    run_name: String,
    //#[derivative(PartialEq = "ignore")]
    savestate_every: Option<Duration>,
}

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
        self.parking.add_parked_car(ParkedCar::new(
            vehicle.make(CarID::tmp_new(
                self.spawner.car_id_counter,
                VehicleType::Car,
            )),
            spot,
            owner,
        ));
        self.spawner.car_id_counter += 1;
    }

    pub fn get_parked_cars_by_owner(&self, bldg: BuildingID) -> Vec<&ParkedCar> {
        self.parking.get_parked_cars_by_owner(bldg)
    }
}

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

impl Sim {
    pub fn draw_unzoomed(&self, g: &mut GfxCtx, map: &Map) {
        self.driving.draw_unzoomed(self.time, g, map);
    }
}

impl Sim {
    pub fn step(&mut self, map: &Map) -> Vec<Event> {
        self.time += Duration::seconds(0.1);

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

        Vec::new()
    }
}

impl Sim {
    pub fn time(&self) -> Duration {
        self.time
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

impl Sim {
    pub fn is_done(&self) -> bool {
        self.spawner.is_done() && self.trips.is_done()
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
}

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

impl Sim {
    pub fn summarize(&self, lanes: &HashSet<LaneID>) -> Summary {
        /*let (cars_parked, open_parking_spots) = self.parking_state.count(lanes);
        let (moving_cars, stuck_cars, buses) = self.driving_state.count(lanes);
        let (moving_peds, stuck_peds) = self.walking_state.count(lanes);*/
        let (cars_parked, open_parking_spots) = (0, 0);
        let (moving_cars, stuck_cars, buses) = (0, 0, 0);
        let (moving_peds, stuck_peds) = (0, 0);

        Summary {
            cars_parked,
            open_parking_spots,
            moving_cars,
            stuck_cars,
            buses,
            moving_peds,
            stuck_peds,
            // Something else has to calculate this
            trips_with_ab_test_divergence: 0,
        }
    }

    // TODO deprecate this, use the new Summary
    pub fn summary(&self) -> String {
        /*let (waiting_cars, active_cars) = self.driving_state.get_active_and_waiting_count();
        let (waiting_peds, active_peds) = self.walking_state.get_active_and_waiting_count();*/
        let (waiting_cars, active_cars) = (0, 0);
        let (waiting_peds, active_peds) = (0, 0);
        format!(
            "Time: {0}, {1} / {2} active cars waiting, {3} cars parked, {4} / {5} pedestrians waiting",
            self.time,
            waiting_cars,
            active_cars,
            0,
            //self.parking_state.total_count(),
            waiting_peds, active_peds,
        )
    }

    pub fn get_score(&self) -> ScoreSummary {
        panic!("TODO");
        /*let mut s = self.trips_state.get_score(self.time);
        if self.is_done() {
            s.completion_time = Some(self.time);
        }
        s*/
    }
}
