// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use control::ControlMap;
use dimensioned::si;
use draw_car::DrawCar;
use draw_ped::DrawPedestrian;
use driving;
use intersections::{AgentInfo, IntersectionSimState};
use map_model::{IntersectionID, LaneID, LaneType, Map, Turn, TurnID};
use parametric_driving;
use parking::ParkingSimState;
use rand::{FromEntropy, Rng, SeedableRng, XorShiftRng};
use spawn::Spawner;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64;
use std::time::{Duration, Instant};
use walking::WalkingSimState;
use {CarID, CarState, Distance, InvariantViolated, PedestrianID, Tick, TIMESTEP};

#[derive(Serialize, Deserialize, Derivative, PartialEq, Eq)]
pub enum DrivingModel {
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

    // Public, mutable, arguments, return type
    (pub fn $fxn_name:ident(&mut self, $($value:ident: $type:ty),* ) -> $ret:ty) => {
        pub fn $fxn_name(&mut self, $( $value: $type ),*) -> $ret {
            match self {
                DrivingModel::V1(s) => s.$fxn_name($( $value ),*),
                DrivingModel::V2(s) => s.$fxn_name($( $value ),*),
            }
        }
    };

    // TODO hack, hardcoding the generic type bounds, because I can't figure it out :(
    // Mutable, arguments, return type
    (fn $fxn_name:ident<R: Rng + ?Sized>(&mut self, $($value:ident: $type:ty),* ) -> $ret:ty) => {
        fn $fxn_name<R: Rng + ?Sized>(&mut self, $( $value: $type ),*) -> $ret {
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
    delegate!(fn step<R: Rng + ?Sized>(&mut self, time: Tick, map: &Map, parking: &ParkingSimState, intersections: &mut IntersectionSimState, rng: &mut R) -> Result<Vec<CarParking>, InvariantViolated>);
    delegate!(pub fn start_car_on_lane(
        &mut self,
        time: Tick,
        car: CarID,
        parking: CarParking,
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

    spawner: Spawner,
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
            spawner: Spawner::empty(),
            intersection_state: IntersectionSimState::new(map),
            parking_state: ParkingSimState::new(map),
            walking_state: WalkingSimState::new(),
            time: Tick::zero(),
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
        self.spawner
            .seed_parked_cars(percent, &mut self.parking_state, &mut self.rng);
    }

    pub fn start_many_parked_cars(&mut self, map: &Map, num_cars: usize) {
        self.spawner.start_many_parked_cars(
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

    pub fn spawn_pedestrian(&mut self, map: &Map, sidewalk: LaneID) {
        self.spawner
            .spawn_pedestrian(self.time.next(), map, sidewalk, &mut self.rng);
    }

    pub fn seed_pedestrians(&mut self, map: &Map, num: usize) {
        self.spawner
            .spawn_many_pedestrians(self.time.next(), map, num, &mut self.rng);
    }

    pub fn step(&mut self, map: &Map, control_map: &ControlMap) {
        self.time = self.time.next();

        self.spawner.step(
            self.time,
            map,
            &mut self.parking_state,
            &mut self.walking_state,
            &mut self.driving_state,
        );

        match self.driving_state.step(
            self.time,
            map,
            &self.parking_state,
            &mut self.intersection_state,
            &mut self.rng,
        ) {
            Ok(parked_cars) => for p in parked_cars {
                self.parking_state.add_parked_car(p);
            },
            Err(e) => panic!("At {}: {}", self.time, e),
        };

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

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct ParkingSpot {
    pub parking_lane: LaneID,
    pub spot_idx: usize,
    // Of the front of the car
    #[derivative(PartialEq = "ignore")]
    pub dist_along: Distance,
}

// TODO better name?
#[derive(PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CarParking {
    pub car: CarID,
    pub spot: ParkingSpot,
}

impl CarParking {
    pub fn new(car: CarID, spot: ParkingSpot) -> CarParking {
        CarParking { car, spot }
    }
}
