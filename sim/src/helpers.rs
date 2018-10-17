use abstutil;
use control::ControlMap;
use flame;
use map_model::{BuildingID, BusRoute, BusStopID, LaneID, Map};
use std::collections::VecDeque;
use {
    CarID, Event, MapEdits, PedestrianID, RouteID, Scenario, SeedParkedCars, Sim, SpawnOverTime,
    Tick, WeightedUsizeChoice,
};

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
            load: "../data/maps/montlake.abst".to_string(),
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
            // TODO or the scenario name if no run name
            flags.run_name,
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
        let sim = Sim::new(&map, flags.run_name, flags.rng_seed, savestate_every);
        flame::end("create sim");
        (map, control_map, sim)
    }
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
        Scenario {
            scenario_name: "small_spawn".to_string(),
            map_name: map.get_name().to_string(),
            seed_parked_cars: vec![SeedParkedCars {
                neighborhood: "_everywhere_".to_string(),
                cars_per_building: WeightedUsizeChoice {
                    weights: vec![5, 5],
                },
            }],
            spawn_over_time: vec![SpawnOverTime {
                num_agents: 100,
                start_tick: Tick::zero(),
                stop_tick: Tick::from_seconds(5),
                start_from_neighborhood: "_everywhere_".to_string(),
                go_to_neighborhood: "_everywhere_".to_string(),
            }],
        }.instantiate(self, map);

        for route in map.get_all_bus_routes() {
            self.seed_bus_route(route, map);
        }

        /*self.make_ped_using_bus(
            map,
            LaneID(550),
            LaneID(727),
            RouteID(0),
            map.get_l(LaneID(325)).bus_stops[0].id,
            map.get_l(LaneID(840)).bus_stops[0].id,
        );*/

        // TODO this is introducing nondeterminism, because of slight floating point errors.
        // fragile that this causes it, but true. :\
    }

    pub fn big_spawn(&mut self, map: &Map) {
        Scenario {
            scenario_name: "big_spawn".to_string(),
            map_name: map.get_name().to_string(),
            seed_parked_cars: vec![SeedParkedCars {
                neighborhood: "_everywhere_".to_string(),
                cars_per_building: WeightedUsizeChoice {
                    weights: vec![2, 8],
                },
            }],
            spawn_over_time: vec![SpawnOverTime {
                num_agents: 1000,
                start_tick: Tick::zero(),
                stop_tick: Tick::from_seconds(5),
                start_from_neighborhood: "_everywhere_".to_string(),
                go_to_neighborhood: "_everywhere_".to_string(),
            }],
        }.instantiate(self, map);
    }

    pub fn seed_parked_cars(
        &mut self,
        owner_buildins: &Vec<BuildingID>,
        cars_per_building: &WeightedUsizeChoice,
        map: &Map,
    ) {
        self.spawner.seed_parked_cars(
            cars_per_building,
            owner_buildins,
            &mut self.parking_state,
            &mut self.rng,
            map,
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
        )
    }

    pub fn seed_specific_parked_cars(
        &mut self,
        lane: LaneID,
        // One owner of many spots, kind of weird, but hey, tests. :D
        owner: BuildingID,
        spots: Vec<usize>,
    ) -> Vec<CarID> {
        self.spawner.seed_specific_parked_cars(
            lane,
            owner,
            spots,
            &mut self.parking_state,
            &mut self.rng,
        )
    }

    pub fn make_ped_using_bus(
        &mut self,
        map: &Map,
        from: BuildingID,
        to: BuildingID,
        route: RouteID,
        stop1: BusStopID,
        stop2: BusStopID,
    ) -> PedestrianID {
        self.spawner.start_trip_using_bus(
            self.time.next(),
            map,
            from,
            to,
            stop1,
            stop2,
            route,
            &mut self.trips_state,
        )
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

    pub fn make_ped_using_car(&mut self, map: &Map, car: CarID, to: BuildingID) {
        let parked = self.parking_state.lookup_car(car).unwrap().clone();
        let owner = parked.owner.unwrap();
        self.spawner.start_trip_using_parked_car(
            self.time.next(),
            map,
            parked,
            &mut self.parking_state,
            owner,
            to,
            &mut self.trips_state,
        );
    }
}
