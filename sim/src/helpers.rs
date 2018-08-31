use abstutil;
use control::ControlMap;
use map_model::{BuildingID, BusStop, Edits, LaneID, Map};
use std::collections::VecDeque;
use {CarID, Event, Sim, Tick};

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
                map.get_l(LaneID(840)).bus_stops[0].clone(),
            ],
            map,
        ).len() != 2
        {
            panic!("Two buses didn't fit");
        }
    }

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

    pub fn spawn_specific_pedestrian(&mut self, map: &Map, from: BuildingID, to: BuildingID) {
        self.spawner
            .spawn_specific_pedestrian(self.time.next(), map, from, to);
    }

    pub fn seed_walking_trips(&mut self, map: &Map, num: usize) {
        self.spawner
            .seed_walking_trips(self.time.next(), map, num, &mut self.rng);
    }
}
