use abstutil;
use control::ControlMap;
use map_model::{BuildingID, BusStop, Edits, LaneID, Map};
use rand::Rng;
use std::collections::VecDeque;
use {CarID, Event, PedestrianID, RouteID, Sim, Tick};

// Convenience method to setup everything.
pub fn load(
    input: String,
    scenario_name: String,
    rng_seed: Option<u8>,
    savestate_every: Option<Tick>,
) -> (Map, Edits, ControlMap, Sim) {
    let edits: Edits = abstutil::read_json("road_edits.json").unwrap_or(Edits::new());

    if input.contains("data/save/") {
        println!("Resuming from {}", input);
        let sim: Sim = abstutil::read_json(&input).expect("loading sim state failed");
        // TODO assuming the relative path :(
        let map_path = format!("../data/{}.abst", sim.map_name);
        let map =
            Map::new(&map_path, &edits).expect(&format!("Couldn't load map from {}", map_path));
        let control_map = ControlMap::new(&map);
        (map, edits, control_map, sim)
    } else {
        println!("Loading map {}", input);
        let map = Map::new(&input, &edits).expect("Couldn't load map");
        let control_map = ControlMap::new(&map);
        let sim = Sim::new(&map, scenario_name, rng_seed, savestate_every);
        (map, edits, control_map, sim)
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
                println!("{0}, speed = {1:.2}x", self.summary(), speed);
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
                    println!("At {}, met expectation {:?}", self.time, ev);
                    expectations.pop_front();
                    if expectations.is_empty() {
                        return;
                    }
                }
            }
            if self.time.is_multiple_of(Tick::from_minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                println!("{0}, speed = {1:.2}x", self.summary(), speed);
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
        self.seed_parked_cars(0.5);
        self.seed_walking_trips(&map, 100);
        self.seed_driving_trips(&map, 100);

        if self.seed_bus_route(
            vec![
                map.get_l(LaneID(309)).bus_stops[0].clone(),
                map.get_l(LaneID(325)).bus_stops[0].clone(),
                map.get_l(LaneID(840)).bus_stops[0].clone(),
            ],
            map,
        ).len() != 3
        {
            panic!("Three buses didn't fit");
        }
        // TODO this is introducing nondeterminism?!
        /*self.make_ped_using_bus(
            map,
            LaneID(550),
            LaneID(727),
            RouteID(0),
            map.get_l(LaneID(325)).bus_stops[0].clone(),
            map.get_l(LaneID(840)).bus_stops[0].clone(),
        );*/    }

    pub fn big_spawn(&mut self, map: &Map) {
        self.seed_parked_cars(0.95);
        self.seed_walking_trips(&map, 1000);
        self.seed_driving_trips(&map, 1000);
    }

    pub fn seed_parked_cars(&mut self, percent: f64) {
        for v in self.spawner
            .seed_parked_cars(percent, &mut self.parking_state, &mut self.rng)
            .into_iter()
        {
            self.car_properties.insert(v.id, v);
        }
    }

    pub fn seed_bus_route(&mut self, stops: Vec<BusStop>, map: &Map) -> Vec<CarID> {
        // TODO throw away the events? :(
        let mut events: Vec<Event> = Vec::new();
        let mut result: Vec<CarID> = Vec::new();
        for v in self.spawner.seed_bus_route(
            &mut events,
            stops,
            &mut self.rng,
            map,
            &mut self.driving_state,
            &mut self.transit_state,
            self.time,
            &self.car_properties,
        ) {
            let id = v.id;
            self.car_properties.insert(v.id, v);
            result.push(id);
        }
        result
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
        let mut cars: Vec<CarID> = self.parking_state
            .get_all_parked_cars()
            .into_iter()
            .filter_map(|parked_car| {
                let lane = parked_car.spot.lane;
                let has_bldgs = map.get_parent(lane)
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
            })
            .collect();
        if cars.is_empty() {
            return;
        }
        self.rng.shuffle(&mut cars);

        for car in &cars[0..num_cars.min(cars.len())] {
            self.start_parked_car(map, *car);
        }
    }

    pub fn start_parked_car(&mut self, map: &Map, car: CarID) {
        let parking_lane = self.parking_state
            .lookup_car(car)
            .expect("Car isn't parked")
            .spot
            .lane;
        let road = map.get_parent(parking_lane);
        let driving_lane = road.find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        let goal = pick_car_goal(&mut self.rng, map, driving_lane);
        self.start_parked_car_with_goal(map, car, goal);
    }

    pub fn start_parked_car_with_goal(&mut self, map: &Map, car: CarID, goal: LaneID) {
        let parked = self.parking_state
            .lookup_car(car)
            .expect("Car isn't parked");
        let road = map.get_parent(parked.spot.lane);
        let sidewalk = road.find_sidewalk(parked.spot.lane)
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
        stop1: BusStop,
        stop2: BusStop,
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
        self.spawner.spawn_specific_pedestrian(
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
    let candidate_goals: Vec<LaneID> = map.all_lanes()
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
        })
        .collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn pick_ped_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> BuildingID {
    let candidate_goals: Vec<BuildingID> = map.all_buildings()
        .iter()
        .filter_map(|b| {
            if b.front_path.sidewalk != start {
                Some(b.id)
            } else {
                None
            }
        })
        .collect();
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
