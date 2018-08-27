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
use walking::WalkingSimState;
use {CarID, CarState, ParkedCar, PedestrianID, Tick, TIMESTEP};

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Sim {
    // This is slightly dangerous, but since we'll be using comparisons based on savestating (which
    // captures the RNG), this should be OK for now.
    #[derivative(PartialEq = "ignore")]
    rng: XorShiftRng,
    pub time: Tick,
    map_name: String,
    scenario_name: String,

    spawner: Spawner,
    intersection_state: IntersectionSimState,
    driving_state: DrivingSimState,
    parking_state: ParkingSimState,
    walking_state: WalkingSimState,

    car_properties: BTreeMap<CarID, Vehicle>,
}

impl Sim {
    pub fn new(map: &Map, scenario_name: String, rng_seed: Option<u8>) -> Sim {
        let mut rng = XorShiftRng::from_entropy();
        if let Some(seed) = rng_seed {
            rng = XorShiftRng::from_seed([seed; 16]);
        }

        Sim {
            rng,
            driving_state: DrivingSimState::new(map),
            spawner: Spawner::empty(),
            intersection_state: IntersectionSimState::new(map),
            parking_state: ParkingSimState::new(map),
            walking_state: WalkingSimState::new(),
            time: Tick::zero(),
            map_name: map.get_name().to_string(),
            scenario_name,
            car_properties: BTreeMap::new(),
        }
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

    pub fn seed_parked_cars(&mut self, percent: f64) {
        for v in self.spawner
            .seed_parked_cars(percent, &mut self.parking_state, &mut self.rng)
            .into_iter()
        {
            self.car_properties.insert(v.id, v);
        }
    }

    pub fn seed_specific_parked_cars(&mut self, lane: LaneID, spots: Vec<usize>) -> Vec<CarID> {
        let mut ids = Vec::new();
        for v in self.spawner
            .seed_specific_parked_cars(lane, spots, &mut self.parking_state, &mut self.rng)
            .into_iter()
        {
            ids.push(v.id);
            self.car_properties.insert(v.id, v);
        }
        ids
    }

    pub fn seed_driving_trips(&mut self, map: &Map, num_cars: usize) {
        self.spawner.seed_driving_trips(
            self.time.next(),
            map,
            num_cars,
            &mut self.rng,
            &self.parking_state,
        );
    }

    pub fn start_parked_car(&mut self, map: &Map, car: CarID) {
        self.spawner.start_parked_car(
            self.time.next(),
            map,
            car,
            &self.parking_state,
            &mut self.rng,
        );
    }

    pub fn start_parked_car_with_goal(&mut self, map: &Map, car: CarID, goal: LaneID) {
        self.spawner.start_parked_car_with_goal(
            self.time.next(),
            map,
            car,
            &self.parking_state,
            goal,
            &mut self.rng,
        );
    }

    pub fn spawn_pedestrian(&mut self, map: &Map, sidewalk: LaneID) {
        self.spawner
            .spawn_pedestrian(self.time.next(), map, sidewalk, &mut self.rng);
    }

    pub fn seed_walking_trips(&mut self, map: &Map, num: usize) {
        self.spawner
            .seed_walking_trips(self.time.next(), map, num, &mut self.rng);
    }

    // TODO not sure returning info for tests like this is ideal
    pub fn step(&mut self, map: &Map, control_map: &ControlMap) -> Vec<ParkedCar> {
        self.time = self.time.next();

        self.spawner.step(
            self.time,
            map,
            &mut self.parking_state,
            &mut self.walking_state,
            &mut self.driving_state,
            &self.car_properties,
        );

        let mut cars_parked_this_step: Vec<ParkedCar> = Vec::new();
        match self.driving_state.step(
            self.time,
            map,
            &self.parking_state,
            &mut self.intersection_state,
            &mut self.rng,
            &self.car_properties,
        ) {
            Ok(parked_cars) => for p in parked_cars {
                cars_parked_this_step.push(p.clone());
                self.parking_state.add_parked_car(p.clone());
                self.spawner
                    .car_reached_parking_spot(self.time, p, map, &self.parking_state);
            },
            Err(e) => panic!("At {}: {}", self.time, e),
        };

        match self.walking_state
            .step(TIMESTEP, map, &mut self.intersection_state)
        {
            Ok(peds_ready_to_drive) => for (ped, spot) in peds_ready_to_drive {
                self.spawner
                    .ped_reached_parking_spot(self.time, ped, spot, &self.parking_state);
            },
            Err(e) => panic!("At {}: {}", self.time, e),
        };

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
            .step(self.time, map, control_map, info);

        cars_parked_this_step
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
        let (_, active_cars) = self.driving_state.get_active_and_waiting_count();
        let (_, active_peds) = self.walking_state.get_active_and_waiting_count();
        active_cars == 0 && active_peds == 0
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
