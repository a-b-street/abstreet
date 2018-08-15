// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use control::ControlMap;
use dimensioned::si;
use draw_car::DrawCar;
use draw_ped::DrawPedestrian;
use driving;
use intersections::{AgentInfo, IntersectionSimState};
use map_model;
use map_model::{IntersectionID, LaneID, LaneType, Map, Turn, TurnID};
use parametric_driving;
use parking::ParkingSimState;
use rand::{FromEntropy, Rng, SeedableRng, XorShiftRng};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64;
use std::time::{Duration, Instant};
use walking::WalkingSimState;
use {CarID, CarState, Distance, InvariantViolated, PedestrianID, Tick, TIMESTEP};

#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
enum DrivingModel {
    V1(driving::DrivingSimState),
    V2(parametric_driving::DrivingSimState),
}

macro_rules! delegate {
    // Immutable, no arguments, return type
    (fn $fxn_name:ident(&self) -> $ret:ty) => {
        fn $fxn_name(&self) -> $ret {
            match self {
                DrivingModel::V1(s) => s.$fxn_name(),
                DrivingModel::V2(s) => s.$fxn_name(),
            }
        }
    };

    // Immutable, arguments, return type
    (fn $fxn_name:ident(&self, $($value:ident: $type:ty),* ) -> $ret:ty) => {
        fn $fxn_name(&self, $( $value: $type ),*) -> $ret {
            match self {
                DrivingModel::V1(s) => s.$fxn_name($( $value ),*),
                DrivingModel::V2(s) => s.$fxn_name($( $value ),*),
            }
        }
    };

    // Immutable, arguments, no return type
    (fn $fxn_name:ident(&self, $($value:ident: $type:ty),* )) => {
        fn $fxn_name(&self, $( $value: $type ),*) {
            match self {
                DrivingModel::V1(s) => s.$fxn_name($( $value ),*),
                DrivingModel::V2(s) => s.$fxn_name($( $value ),*),
            }
        }
    };

    // Mutable, arguments, return type
    (fn $fxn_name:ident(&mut self, $($value:ident: $type:ty),* ) -> $ret:ty) => {
        fn $fxn_name(&mut self, $( $value: $type ),*) -> $ret {
            match self {
                DrivingModel::V1(s) => s.$fxn_name($( $value ),*),
                DrivingModel::V2(s) => s.$fxn_name($( $value ),*),
            }
        }
    };

    // Mutable, arguments, no return type
    (fn $fxn_name:ident(&mut self, $($value:ident: $type:ty),* )) => {
        fn $fxn_name(&mut self, $( $value: $type ),*) {
            match self {
                DrivingModel::V1(s) => s.$fxn_name($( $value ),*),
                DrivingModel::V2(s) => s.$fxn_name($( $value ),*),
            }
        }
    };
}

impl DrivingModel {
    delegate!(fn populate_info_for_intersections(&self, info: &mut AgentInfo, map: &Map));
    delegate!(fn get_car_state(&self, c: CarID) -> CarState);
    delegate!(fn get_active_and_waiting_count(&self) -> (usize, usize));
    delegate!(fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>>);
    delegate!(fn toggle_debug(&mut self, id: CarID));
    delegate!(fn edit_remove_lane(&mut self, id: LaneID));
    delegate!(fn edit_add_lane(&mut self, id: LaneID));
    delegate!(fn edit_remove_turn(&mut self, id: TurnID));
    delegate!(fn edit_add_turn(&mut self, id: TurnID, map: &Map));
    delegate!(fn step(&mut self, time: Tick, map: &Map, intersections: &mut IntersectionSimState) -> Result<(), InvariantViolated>);
    delegate!(fn start_car_on_lane(
        &mut self,
        time: Tick,
        car: CarID,
        dist_along: Distance,
        path: VecDeque<LaneID>,
        map: &Map
    ) -> bool);
    delegate!(fn get_draw_car(&self, id: CarID, time: Tick, map: &Map) -> Option<DrawCar>);
    delegate!(fn get_draw_cars_on_lane(&self, lane: LaneID, time: Tick, map: &Map) -> Vec<DrawCar>);
    delegate!(fn get_draw_cars_on_turn(&self, turn: TurnID, time: Tick, map: &Map) -> Vec<DrawCar>);
}

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Sim {
    // This is slightly dangerous, but since we'll be using comparisons based on savestating (which
    // captures the RNG), this should be OK for now.
    #[derivative(PartialEq = "ignore")]
    rng: XorShiftRng,
    pub time: Tick,
    car_id_counter: usize,

    intersection_state: IntersectionSimState,
    driving_state: DrivingModel,
    parking_state: ParkingSimState,
    walking_state: WalkingSimState,
}

impl Sim {
    pub fn new(map: &Map, rng_seed: Option<u8>, parametric_sim: bool) -> Sim {
        let mut rng = XorShiftRng::from_entropy();
        if let Some(seed) = rng_seed {
            rng = XorShiftRng::from_seed([seed; 16]);
        }

        let driving_state = if parametric_sim {
            DrivingModel::V2(parametric_driving::DrivingSimState::new(map))
        } else {
            DrivingModel::V1(driving::DrivingSimState::new(map))
        };

        Sim {
            rng,
            driving_state,
            intersection_state: IntersectionSimState::new(map),
            parking_state: ParkingSimState::new(map),
            walking_state: WalkingSimState::new(),
            time: Tick::zero(),
            car_id_counter: 0,
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
        self.parking_state
            .seed_random_cars(&mut self.rng, percent, &mut self.car_id_counter)
    }

    pub fn start_many_parked_cars(&mut self, map: &Map, num_cars: usize) {
        use rayon::prelude::*;

        let mut cars_and_starts: Vec<(CarID, LaneID)> = self.parking_state
            .get_all_cars()
            .into_iter()
            .filter_map(|(car, parking_lane)| {
                map.get_parent(parking_lane)
                    .find_driving_lane(parking_lane)
                    .and_then(|driving_lane| Some((car, driving_lane)))
            })
            .collect();
        if cars_and_starts.is_empty() {
            return;
        }
        self.rng.shuffle(&mut cars_and_starts);

        let driving_lanes: Vec<LaneID> = map.all_lanes()
            .iter()
            .filter_map(|l| if l.is_driving() { Some(l.id) } else { None })
            .collect();
        let mut requested_paths: Vec<(CarID, LaneID, LaneID)> = Vec::new();
        for i in 0..num_cars.min(cars_and_starts.len()) {
            let (car, start) = cars_and_starts[i];
            let goal = choose_different(&mut self.rng, &driving_lanes, start);
            requested_paths.push((car, start, goal));
        }

        println!("Calculating {} paths for cars", requested_paths.len());
        let timer = Instant::now();
        let paths: Vec<(CarID, Option<Vec<LaneID>>)> = requested_paths
            .par_iter()
            .map(|(car, start, goal)| (*car, map_model::pathfind(map, *start, *goal)))
            .collect();

        let mut actual = 0;
        for (car, path) in paths.into_iter() {
            if let Some(steps) = path {
                if self.start_parked_car_with_path(car, map, steps) {
                    actual += 1;
                }
            } else {
                println!("Failed to pathfind for {}", car);
            };
        }

        println!(
            "Calculating {} car paths took {:?}",
            requested_paths.len(),
            timer.elapsed()
        );
        println!("Started {} parked cars of requested {}", actual, num_cars);
    }

    fn start_parked_car_with_path(&mut self, car: CarID, map: &Map, steps: Vec<LaneID>) -> bool {
        let driving_lane = steps[0];
        let parking_lane = map.get_parent(driving_lane)
            .find_parking_lane(driving_lane)
            .unwrap();
        let dist_along = self.parking_state.get_dist_along_lane(car, parking_lane);

        if self.driving_state.start_car_on_lane(
            self.time,
            car,
            dist_along,
            VecDeque::from(steps),
            map,
        ) {
            self.parking_state.remove_parked_car(parking_lane, car);
            true
        } else {
            false
        }
    }

    pub fn start_parked_car(&mut self, map: &Map, car: CarID) -> bool {
        let parking_lane = self.parking_state
            .lane_of_car(car)
            .expect("Car isn't parked");
        let road = map.get_parent(parking_lane);
        let driving_lane = road.find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        if let Some(path) = pick_goal_and_find_path(&mut self.rng, map, driving_lane) {
            self.start_parked_car_with_path(car, map, path)
        } else {
            false
        }
    }

    pub fn spawn_pedestrian(&mut self, map: &Map, sidewalk: LaneID) -> bool {
        assert!(map.get_l(sidewalk).is_sidewalk());

        if let Some(path) = pick_goal_and_find_path(&mut self.rng, map, sidewalk) {
            self.walking_state
                .seed_pedestrian(map, VecDeque::from(path));
            println!("Spawned a pedestrian at {}", sidewalk);
            true
        } else {
            false
        }
    }

    pub fn seed_pedestrians(&mut self, map: &Map, num: usize) {
        use rayon::prelude::*;

        let mut sidewalks: Vec<LaneID> = Vec::new();
        for l in map.all_lanes() {
            if l.is_sidewalk() {
                sidewalks.push(l.id);
            }
        }

        let mut requested_paths: Vec<(LaneID, LaneID)> = Vec::new();
        for _i in 0..num {
            let start = *self.rng.choose(&sidewalks).unwrap();
            let goal = choose_different(&mut self.rng, &sidewalks, start);
            requested_paths.push((start, goal));
        }

        println!("Calculating {} paths for pedestrians", num);
        // TODO better timer macro
        let timer = Instant::now();
        let paths: Vec<Option<Vec<LaneID>>> = requested_paths
            .par_iter()
            .map(|(start, goal)| map_model::pathfind(map, *start, *goal))
            .collect();

        let mut actual = 0;
        for path in paths.into_iter() {
            if let Some(steps) = path {
                self.walking_state
                    .seed_pedestrian(map, VecDeque::from(steps));
                actual += 1;
            } else {
                // zip with request to have start/goal?
                //println!("Failed to pathfind for a pedestrian");
            };
        }

        println!(
            "Calculating {} pedestrian paths took {:?}",
            num,
            timer.elapsed()
        );
        println!("Spawned {} pedestrians of requested {}", actual, num);
    }

    pub fn step(&mut self, map: &Map, control_map: &ControlMap) {
        self.time.increment();

        // TODO Vanish action should become Park
        if let Err(e) = self.driving_state
            .step(self.time, map, &mut self.intersection_state)
        {
            panic!("At {}: {}", self.time, e);
        }
        if let Err(e) = self.walking_state
            .step(TIMESTEP, map, &mut self.intersection_state)
        {
            panic!("At {}: {}", self.time, e);
        }

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
    }

    pub fn get_car_state(&self, c: CarID) -> CarState {
        self.driving_state.get_car_state(c)
    }

    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCar> {
        self.driving_state
            .get_draw_car(id, self.time, map)
            .or_else(|| self.parking_state.get_draw_car(id, map))
    }

    pub fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrian> {
        self.walking_state.get_draw_ped(id, map)
    }

    // TODO maybe just DrawAgent instead? should caller care?
    pub fn get_draw_cars_on_lane(&self, l: LaneID, map: &Map) -> Vec<DrawCar> {
        match map.get_l(l).lane_type {
            LaneType::Driving => self.driving_state.get_draw_cars_on_lane(l, self.time, map),
            LaneType::Parking => self.parking_state.get_draw_cars(l, map),
            LaneType::Sidewalk => Vec::new(),
            LaneType::Biking => Vec::new(),
        }
    }

    pub fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCar> {
        self.driving_state.get_draw_cars_on_turn(t, self.time, map)
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
            "Time: {0:.2}, {1} / {2} active cars waiting, {3} cars parked, {4} / {5} pedestrians waiting",
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

    pub fn debug_intersection(&self, id: IntersectionID) {
        self.intersection_state.debug(id);
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

fn choose_different<R: Rng + ?Sized, T: PartialEq + Copy>(
    rng: &mut R,
    choices: &Vec<T>,
    except: T,
) -> T {
    assert!(choices.len() > 1);
    loop {
        let choice = *rng.choose(choices).unwrap();
        if choice != except {
            return choice;
        }
    }
}

fn pick_goal_and_find_path<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    start: LaneID,
) -> Option<Vec<LaneID>> {
    let lane_type = map.get_l(start).lane_type;
    let candidate_goals: Vec<LaneID> = map.all_lanes()
        .iter()
        .filter_map(|l| {
            if l.lane_type != lane_type || l.id == start {
                None
            } else {
                Some(l.id)
            }
        })
        .collect();
    let goal = rng.choose(&candidate_goals).unwrap();
    if let Some(steps) = map_model::pathfind(map, start, *goal) {
        Some(steps)
    } else {
        println!("No path from {} to {} ({:?})", start, goal, lane_type);
        None
    }
}
