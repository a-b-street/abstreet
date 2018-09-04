// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use control::ControlMap;
use dimensioned::si;
use draw_car::DrawCar;
use draw_ped::DrawPedestrian;
use driving::DrivingSimState;
use intersections::{AgentInfo, IntersectionSimState};
use kinematics::Vehicle;
use map_model::{IntersectionID, LaneID, LaneType, Map, Turn, TurnID};
use parking::ParkingSimState;
use rand::{FromEntropy, SeedableRng, XorShiftRng};
use spawn::Spawner;
use std;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::f64;
use std::time::{Duration, Instant};
use transit::TransitSimState;
use trips::TripManager;
use walking::WalkingSimState;
use {AgentID, CarID, CarState, Event, InvariantViolated, PedestrianID, Tick, TIMESTEP};

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Sim {
    // TODO all the pub(crate) stuff is for helpers. Find a better solution.

    // This is slightly dangerous, but since we'll be using comparisons based on savestating (which
    // captures the RNG), this should be OK for now.
    #[derivative(PartialEq = "ignore")]
    pub(crate) rng: XorShiftRng,
    pub time: Tick,
    pub(crate) map_name: String,
    // Some tests deliberately set different scenario names for comparisons.
    #[derivative(PartialEq = "ignore")]
    scenario_name: String,
    // TODO not quite the right type to represent durations
    savestate_every: Option<Tick>,

    pub(crate) spawner: Spawner,
    intersection_state: IntersectionSimState,
    pub(crate) driving_state: DrivingSimState,
    pub(crate) parking_state: ParkingSimState,
    pub(crate) walking_state: WalkingSimState,
    pub(crate) transit_state: TransitSimState,
    pub(crate) trips_state: TripManager,

    pub(crate) car_properties: BTreeMap<CarID, Vehicle>,
}

impl Sim {
    // TODO Options struct might be nicer, especially since we could glue it to structopt?
    pub fn new(
        map: &Map,
        scenario_name: String,
        rng_seed: Option<u8>,
        savestate_every: Option<Tick>,
    ) -> Sim {
        let mut rng = XorShiftRng::from_entropy();
        if let Some(seed) = rng_seed {
            rng = XorShiftRng::from_seed([seed; 16]);
        }

        Sim {
            rng,
            driving_state: DrivingSimState::new(map),
            spawner: Spawner::empty(),
            trips_state: TripManager::new(),
            intersection_state: IntersectionSimState::new(map),
            parking_state: ParkingSimState::new(map),
            walking_state: WalkingSimState::new(),
            transit_state: TransitSimState::new(),
            time: Tick::zero(),
            map_name: map.get_name().to_string(),
            scenario_name,
            savestate_every,
            car_properties: BTreeMap::new(),
        }
    }

    pub fn load(path: String, new_scenario_name: String) -> Result<Sim, std::io::Error> {
        abstutil::read_json(&path).map(|mut s: Sim| {
            s.scenario_name = new_scenario_name;
            s
        })
    }

    pub fn edit_lane_type(&mut self, id: LaneID, old_type: LaneType, map: &Map) {
        match old_type {
            LaneType::Driving => self.driving_state.edit_remove_lane(id),
            LaneType::Parking => self.parking_state.edit_remove_lane(id),
            LaneType::Sidewalk => self.walking_state.edit_remove_lane(id),
            LaneType::Biking => {}
        };
        let l = map.get_l(id);
        match l.lane_type {
            LaneType::Driving => self.driving_state.edit_add_lane(id),
            LaneType::Parking => self.parking_state.edit_add_lane(l),
            LaneType::Sidewalk => self.walking_state.edit_add_lane(id),
            LaneType::Biking => {}
        };
    }

    pub fn edit_remove_turn(&mut self, t: &Turn) {
        if t.between_sidewalks {
            self.walking_state.edit_remove_turn(t.id);
        } else {
            self.driving_state.edit_remove_turn(t.id);
        }
    }

    pub fn edit_add_turn(&mut self, t: &Turn, map: &Map) {
        if t.between_sidewalks {
            self.walking_state.edit_add_turn(t.id);
        } else {
            self.driving_state.edit_add_turn(t.id, map);
        }
    }

    pub fn step(&mut self, map: &Map, control_map: &ControlMap) -> Vec<Event> {
        match self.inner_step(map, control_map) {
            Ok(events) => events,
            Err(e) => panic!(
                "At {}: {}\n\nDebug from {:?}",
                self.time,
                e,
                self.find_most_recent_savestate()
            ),
        }
    }

    fn inner_step(
        &mut self,
        map: &Map,
        control_map: &ControlMap,
    ) -> Result<(Vec<Event>), InvariantViolated> {
        self.time = self.time.next();

        let mut events: Vec<Event> = Vec::new();

        self.spawner.step(
            &mut events,
            self.time,
            map,
            &mut self.parking_state,
            &mut self.walking_state,
            &mut self.driving_state,
            &mut self.trips_state,
            &self.car_properties,
        );

        for p in self.driving_state.step(
            &mut events,
            self.time,
            map,
            &self.parking_state,
            &mut self.intersection_state,
            &mut self.transit_state,
            &mut self.rng,
            &self.car_properties,
        )? {
            events.push(Event::CarReachedParkingSpot(p.clone()));
            self.parking_state.add_parked_car(p.clone());
            self.spawner.car_reached_parking_spot(
                self.time,
                p,
                map,
                &self.parking_state,
                &mut self.trips_state,
            );
        }

        for (ped, spot) in
            self.walking_state
                .step(&mut events, TIMESTEP, map, &mut self.intersection_state)?
        {
            events.push(Event::PedReachedParkingSpot(ped, spot));
            self.spawner.ped_reached_parking_spot(
                self.time,
                ped,
                spot,
                &self.parking_state,
                &mut self.trips_state,
            );
        }

        self.transit_state.step(
            self.time,
            &mut events,
            &mut self.walking_state,
            &mut self.trips_state,
            &mut self.spawner,
            map,
        );

        // TODO want to pass self as a lazy QueryCar trait, but intersection_state is mutably
        // borrowed :(
        let mut info = AgentInfo {
            speeds: HashMap::new(),
            leaders: HashSet::new(),
        };
        self.driving_state
            .populate_info_for_intersections(&mut info, map);
        self.walking_state
            .populate_info_for_intersections(&mut info);

        self.intersection_state
            .step(&mut events, self.time, map, control_map, info);

        // Savestate?
        if let Some(t) = self.savestate_every {
            if self.time.is_multiple_of(t) {
                self.save();
            }
        }

        Ok(events)
    }

    pub fn get_car_state(&self, c: CarID) -> CarState {
        self.driving_state.get_car_state(c)
    }

    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCar> {
        self.driving_state
            .get_draw_car(id, self.time, map, &self.car_properties)
            .or_else(|| {
                self.parking_state
                    .get_draw_car(id, map, &self.car_properties)
            })
    }

    pub fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrian> {
        self.walking_state.get_draw_ped(id, map)
    }

    // TODO maybe just DrawAgent instead? should caller care?
    pub fn get_draw_cars_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawCar> {
        match map.get_l(l).lane_type {
            LaneType::Driving => {
                self.driving_state
                    .get_draw_cars_on_lane(l, self.time, map, &self.car_properties)
            }
            LaneType::Parking => self.parking_state
                .get_draw_cars(l, map, &self.car_properties),
            LaneType::Sidewalk => Vec::new(),
            LaneType::Biking => Vec::new(),
        }
    }

    pub fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCar> {
        self.driving_state
            .get_draw_cars_on_turn(t, self.time, map, &self.car_properties)
    }

    pub fn get_draw_peds_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawPedestrian> {
        self.walking_state.get_draw_peds_on_lane(map.get_l(l), map)
    }

    pub fn get_draw_peds_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawPedestrian> {
        self.walking_state.get_draw_peds_on_turn(map.get_t(t))
    }

    pub fn summary(&self) -> String {
        let (waiting_cars, active_cars) = self.driving_state.get_active_and_waiting_count();
        let (waiting_peds, active_peds) = self.walking_state.get_active_and_waiting_count();
        format!(
            "Time: {0}, {1} / {2} active cars waiting, {3} cars parked, {4} / {5} pedestrians waiting",
            self.time,
            waiting_cars,
            active_cars,
            self.parking_state.total_count(),
            waiting_peds, active_peds,
        )
    }

    pub fn is_done(&self) -> bool {
        self.driving_state.is_done() && self.walking_state.is_done() && self.spawner.is_done()
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        self.walking_state.debug_ped(id);
    }

    pub fn ped_tooltip(&self, p: PedestrianID) -> Vec<String> {
        vec![format!("Hello to {}", p)]
    }

    pub fn car_tooltip(&self, car: CarID) -> Vec<String> {
        self.driving_state
            .tooltip_lines(car)
            .unwrap_or(vec![format!("{} is parked", car)])
    }

    pub fn toggle_debug(&mut self, id: CarID) {
        self.driving_state.toggle_debug(id);
    }

    pub fn start_benchmark(&self) -> Benchmark {
        Benchmark {
            last_real_time: Instant::now(),
            last_sim_time: self.time,
        }
    }

    pub fn measure_speed(&self, b: &mut Benchmark) -> f64 {
        let elapsed = b.last_real_time.elapsed();
        let dt = (elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) * 1e-9) * si::S;
        let speed = (self.time - b.last_sim_time).as_time() / dt;
        b.last_real_time = Instant::now();
        b.last_sim_time = self.time;
        speed.value_unsafe
    }

    pub fn debug_intersection(&mut self, id: IntersectionID, control_map: &ControlMap) {
        self.intersection_state.debug(id, control_map);
    }

    pub fn save(&self) -> String {
        // If we wanted to be even more reproducible, we'd encode RNG seed, version of code, etc,
        // but that's overkill right now.
        let path = format!(
            "../data/save/{}/{}/{}",
            self.map_name,
            self.scenario_name,
            self.time.as_filename()
        );
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap())
            .expect("Creating parent dir failed");
        abstutil::write_json(&path, &self).expect("Writing sim state failed");
        println!("Saved to {}", path);
        path
    }

    pub fn load_most_recent(&self) -> Result<Sim, std::io::Error> {
        let load = self.find_most_recent_savestate()?;
        println!("Loading {}", load);
        abstutil::read_json(&load)
    }

    fn find_most_recent_savestate(&self) -> Result<String, std::io::Error> {
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        for entry in std::fs::read_dir(format!(
            "../data/save/{}/{}/",
            self.map_name, self.scenario_name
        ))? {
            let entry = entry?;
            paths.push(entry.path());
        }
        paths.sort();
        if let Some(p) = paths.last() {
            Ok(p.as_os_str().to_os_string().into_string().unwrap())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "empty directory",
            ))
        }
    }

    pub fn get_current_route(&self, id: AgentID) -> Option<Vec<LaneID>> {
        match id {
            AgentID::Car(car) => self.driving_state.get_current_route(car),
            AgentID::Pedestrian(ped) => self.walking_state.get_current_route(ped),
        }
    }
}

pub struct Benchmark {
    last_real_time: Instant,
    last_sim_time: Tick,
}

impl Benchmark {
    pub fn has_real_time_passed(&self, d: Duration) -> bool {
        self.last_real_time.elapsed() >= d
    }
}
