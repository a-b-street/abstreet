use abstutil;
use control::ControlMap;
use flame;
use geom::Polygon;
use map_model::{BuildingID, BusRoute, BusStopID, LaneID, Map};
use rand::Rng;
use std::collections::VecDeque;
use {CarID, Event, MapEdits, PedestrianID, RouteID, Scenario, Sim, Tick};

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "sim_flags")]
pub struct SimFlags {
    /// Map, scenario, or savestate to load
    #[structopt(name = "load")]
    pub load: String,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    pub rng_seed: Option<u8>,

    /// Run name for savestating
    #[structopt(long = "run_name", default_value = "unnamed")]
    pub run_name: String,

    /// Name of map edits. Shouldn't be a full path or have the ".json"
    #[structopt(long = "edits_name", default_value = "no_edits")]
    pub edits_name: String,
}

impl SimFlags {
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags {
            load: "../data/maps/small.abst".to_string(),
            rng_seed: Some(42),
            run_name: run_name.to_string(),
            edits_name: "no_edits".to_string(),
        }
    }
}

// Convenience method to setup everything.
pub fn load(flags: SimFlags, savestate_every: Option<Tick>) -> (Map, ControlMap, Sim) {
    if flags.load.contains("data/save/") {
        info!("Resuming from {}", flags.load);
        flame::start("read sim savestate");
        let sim: Sim = abstutil::read_json(&flags.load).expect("loading sim state failed");
        flame::end("read sim savestate");
        let edits = load_edits(&sim.map_name, &flags);
        let map_path = format!("../data/maps/{}.abst", sim.map_name);
        let map = Map::new(&map_path, edits.road_edits.clone())
            .expect(&format!("Couldn't load map from {}", map_path));
        let control_map = ControlMap::new(&map, edits.stop_signs, edits.traffic_signals);
        (map, control_map, sim)
    } else if flags.load.contains("data/scenarios/") {
        info!("Seeding the simulation from scenario {}", flags.load);
        let scenario: Scenario = abstutil::read_json(&flags.load).expect("loading scenario failed");
        let edits = load_edits(&scenario.map_name, &flags);
        let map_path = format!("../data/maps/{}.abst", scenario.map_name);
        let map = Map::new(&map_path, edits.road_edits.clone())
            .expect(&format!("Couldn't load map from {}", map_path));
        let control_map = ControlMap::new(&map, edits.stop_signs, edits.traffic_signals);
        let mut sim = Sim::new(
            &map,
            scenario.scenario_name.clone(),
            flags.rng_seed,
            savestate_every,
        );
        scenario.instantiate(&mut sim, &map);
        (map, control_map, sim)
    } else {
        // TODO relative dir is brittle; match more cautiously
        let map_name = flags
            .load
            .trim_left_matches("../data/maps/")
            .trim_right_matches(".abst")
            .to_string();
        info!("Loading map {}", flags.load);
        let edits = load_edits(&map_name, &flags);
        let map = Map::new(&flags.load, edits.road_edits.clone()).expect("Couldn't load map");
        let control_map = ControlMap::new(&map, edits.stop_signs, edits.traffic_signals);
        flame::start("create sim");
        // Note this is the only time we actually use the run name!
        let sim = Sim::new(&map, flags.run_name, flags.rng_seed, savestate_every);
        flame::end("create sim");
        (map, control_map, sim)
    }
}

// Helpers to run the sim
impl Sim {
    // TODO share the helpers for spawning specific parking spots and stuff?

    pub fn run_until_done(&mut self, map: &Map, control_map: &ControlMap, callback: Box<Fn(&Sim)>) {
        let mut benchmark = self.start_benchmark();
        loop {
            self.step(&map, &control_map);
            if self.time.is_multiple_of(Tick::from_minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                info!("{0}, speed = {1:.2}x", self.summary(), speed);
            }
            callback(self);
            if self.is_done() {
                break;
            }
        }
    }

    pub fn run_until_expectations_met(
        &mut self,
        map: &Map,
        control_map: &ControlMap,
        all_expectations: Vec<Event>,
        time_limit: Tick,
    ) {
        let mut benchmark = self.start_benchmark();
        let mut expectations = VecDeque::from(all_expectations);
        loop {
            if expectations.is_empty() {
                return;
            }
            for ev in self.step(&map, &control_map).into_iter() {
                if ev == *expectations.front().unwrap() {
                    info!("At {}, met expectation {:?}", self.time, ev);
                    expectations.pop_front();
                    if expectations.is_empty() {
                        return;
                    }
                }
            }
            if self.time.is_multiple_of(Tick::from_minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                info!("{0}, speed = {1:.2}x", self.summary(), speed);
            }
            if self.time == time_limit {
                panic!(
                    "Time limit {} hit, but some expectations never met: {:?}",
                    self.time, expectations
                );
            }
        }
    }
}

// Spawning helpers
impl Sim {
    pub fn small_spawn(&mut self, map: &Map) {
        self.seed_parked_cars(None, 0.5);
        self.seed_walking_trips(&map, 100);
        self.seed_driving_trips(&map, 100);

        for route in map.get_all_bus_routes() {
            self.seed_bus_route(route, map);
        }
        // TODO this is introducing nondeterminism, because of slight floating point errors.
        // fragile that this causes it, but true. :\
        /*self.make_ped_using_bus(
            map,
            LaneID(550),
            LaneID(727),
            RouteID(0),
            map.get_l(LaneID(325)).bus_stops[0].id,
            map.get_l(LaneID(840)).bus_stops[0].id,
        );*/    }

    pub fn big_spawn(&mut self, map: &Map) {
        self.seed_parked_cars(None, 0.95);
        self.seed_walking_trips(&map, 1000);
        self.seed_driving_trips(&map, 1000);
    }

    pub fn seed_parked_cars(&mut self, in_poly: Option<&Polygon>, percent: f64) {
        self.spawner.seed_parked_cars(
            percent,
            in_poly,
            &mut self.parking_state,
            &mut self.car_properties,
            &mut self.rng,
        );
    }

    pub fn seed_bus_route(&mut self, route: &BusRoute, map: &Map) -> Vec<CarID> {
        // TODO throw away the events? :(
        let mut events: Vec<Event> = Vec::new();
        self.spawner.seed_bus_route(
            &mut events,
            route,
            &mut self.rng,
            map,
            &mut self.driving_state,
            &mut self.transit_state,
            self.time,
            &mut self.car_properties,
        )
    }

    pub fn seed_specific_parked_cars(&mut self, lane: LaneID, spots: Vec<usize>) -> Vec<CarID> {
        self.spawner.seed_specific_parked_cars(
            lane,
            spots,
            &mut self.parking_state,
            &mut self.car_properties,
            &mut self.rng,
        )
    }

    pub fn seed_driving_trips(&mut self, map: &Map, num_cars: usize) {
        let mut cars: Vec<CarID> = self
            .parking_state
            .get_all_parked_cars(None)
            .into_iter()
            .filter_map(|parked_car| {
                let lane = parked_car.spot.lane;
                let has_bldgs = map
                    .get_parent(lane)
                    .find_sidewalk(lane)
                    .and_then(|sidewalk| Some(!map.get_l(sidewalk).building_paths.is_empty()))
                    .unwrap_or(false);
                if has_bldgs {
                    map.get_parent(lane)
                        .find_driving_lane(lane)
                        .and_then(|_driving_lane| Some(parked_car.car))
                } else {
                    None
                }
            }).collect();
        if cars.is_empty() {
            return;
        }
        self.rng.shuffle(&mut cars);

        for car in &cars[0..num_cars.min(cars.len())] {
            self.start_parked_car(map, *car);
        }
    }

    pub fn start_parked_car(&mut self, map: &Map, car: CarID) {
        let parking_lane = self
            .parking_state
            .lookup_car(car)
            .expect("Car isn't parked")
            .spot
            .lane;
        let road = map.get_parent(parking_lane);
        let driving_lane = road
            .find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        let goal = pick_car_goal(&mut self.rng, map, driving_lane);
        self.start_parked_car_with_goal(map, car, goal);
    }

    pub fn start_parked_car_with_goal(&mut self, map: &Map, car: CarID, goal: LaneID) {
        let parked = self
            .parking_state
            .lookup_car(car)
            .expect("Car isn't parked");
        let road = map.get_parent(parked.spot.lane);
        let sidewalk = road
            .find_sidewalk(parked.spot.lane)
            .expect("Parking lane has no sidewalk");

        let start_bldg = pick_bldg_from_sidewalk(&mut self.rng, map, sidewalk);
        let goal_bldg = pick_bldg_from_driving_lane(&mut self.rng, map, goal);

        self.spawner.start_trip_using_parked_car(
            self.time.next(),
            map,
            parked,
            &self.parking_state,
            start_bldg,
            goal_bldg,
            &mut self.trips_state,
        );
    }

    pub fn make_ped_using_bus(
        &mut self,
        map: &Map,
        from: LaneID,
        to: LaneID,
        route: RouteID,
        stop1: BusStopID,
        stop2: BusStopID,
    ) -> PedestrianID {
        let start_bldg = pick_bldg_from_sidewalk(&mut self.rng, map, from);
        let goal_bldg = pick_bldg_from_sidewalk(&mut self.rng, map, to);

        self.spawner.start_trip_using_bus(
            self.time.next(),
            map,
            start_bldg,
            goal_bldg,
            stop1,
            stop2,
            route,
            &mut self.trips_state,
        )
    }

    pub fn spawn_pedestrian(&mut self, map: &Map, sidewalk: LaneID) {
        assert!(map.get_l(sidewalk).is_sidewalk());
        let start_bldg = pick_bldg_from_sidewalk(&mut self.rng, map, sidewalk);
        let goal_bldg = pick_ped_goal(&mut self.rng, map, sidewalk);
        self.spawn_specific_pedestrian(map, start_bldg, goal_bldg);
    }

    pub fn spawn_specific_pedestrian(&mut self, map: &Map, from: BuildingID, to: BuildingID) {
        self.spawner.start_trip_just_walking(
            self.time.next(),
            map,
            from,
            to,
            &mut self.trips_state,
        );
    }

    pub fn seed_walking_trips(&mut self, map: &Map, num: usize) {
        let mut sidewalks: Vec<LaneID> = Vec::new();
        for l in map.all_lanes() {
            if l.is_sidewalk() && !l.building_paths.is_empty() {
                sidewalks.push(l.id);
            }
        }

        for _i in 0..num {
            let start = *self.rng.choose(&sidewalks).unwrap();
            self.spawn_pedestrian(map, start);
        }
    }
}

fn pick_car_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> LaneID {
    let candidate_goals: Vec<LaneID> = map
        .all_lanes()
        .iter()
        .filter_map(|l| {
            if l.id != start && l.is_driving() {
                if let Some(sidewalk) = map.get_sidewalk_from_driving_lane(l.id) {
                    if !map.get_l(sidewalk).building_paths.is_empty() {
                        return Some(l.id);
                    }
                }
            }
            None
        }).collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn pick_ped_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> BuildingID {
    let candidate_goals: Vec<BuildingID> = map
        .all_buildings()
        .iter()
        .filter_map(|b| {
            if b.front_path.sidewalk != start {
                Some(b.id)
            } else {
                None
            }
        }).collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn pick_bldg_from_sidewalk<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    sidewalk: LaneID,
) -> BuildingID {
    *rng.choose(&map.get_l(sidewalk).building_paths)
        .expect(&format!("{} has no buildings", sidewalk))
}

fn pick_bldg_from_driving_lane<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    start: LaneID,
) -> BuildingID {
    pick_bldg_from_sidewalk(rng, map, map.get_sidewalk_from_driving_lane(start).unwrap())
}

fn load_edits(map_name: &str, flags: &SimFlags) -> MapEdits {
    if flags.edits_name == "no_edits" {
        return MapEdits::new();
    }
    if flags.edits_name.contains("data/") || flags.edits_name.contains(".json") {
        panic!(
            "{} should just be a plain name, not a full path",
            flags.edits_name
        );
    }
    let edits: MapEdits = abstutil::read_json(&format!(
        "../data/edits/{}/{}.json",
        map_name, flags.edits_name
    )).unwrap();
    edits
}
