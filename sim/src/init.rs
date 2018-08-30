use abstutil;
use control::ControlMap;
use map_model::{Edits, LaneID, Map};
use std::collections::VecDeque;
use {Event, Sim, Tick};

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

pub fn small_spawn(sim: &mut Sim, map: &Map) {
    sim.seed_parked_cars(0.5);
    sim.seed_walking_trips(&map, 100);
    sim.seed_driving_trips(&map, 100);

    if sim.seed_bus_route(
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

pub fn big_spawn(sim: &mut Sim, map: &Map) {
    sim.seed_parked_cars(0.95);
    sim.seed_walking_trips(&map, 1000);
    sim.seed_driving_trips(&map, 1000);
}

// TODO share the helpers for spawning specific parking spots and stuff?

pub fn run_until_done(sim: &mut Sim, map: &Map, control_map: &ControlMap, callback: Box<Fn(&Sim)>) {
    let mut benchmark = sim.start_benchmark();
    loop {
        sim.step(&map, &control_map);
        if sim.time.is_multiple_of(Tick::from_minutes(1)) {
            let speed = sim.measure_speed(&mut benchmark);
            println!("{0}, speed = {1:.2}x", sim.summary(), speed);
        }
        callback(sim);
        if sim.is_done() {
            break;
        }
    }
}

pub fn run_until_expectations_met(
    sim: &mut Sim,
    map: &Map,
    control_map: &ControlMap,
    all_expectations: Vec<Event>,
    time_limit: Tick,
) {
    let mut benchmark = sim.start_benchmark();
    let mut expectations = VecDeque::from(all_expectations);
    loop {
        if expectations.is_empty() {
            return;
        }
        for ev in sim.step(&map, &control_map).into_iter() {
            if ev == *expectations.front().unwrap() {
                println!("At {}, met expectation {:?}", sim.time, ev);
                expectations.pop_front();
                if expectations.is_empty() {
                    return;
                }
            }
        }
        if sim.time.is_multiple_of(Tick::from_minutes(1)) {
            let speed = sim.measure_speed(&mut benchmark);
            println!("{0}, speed = {1:.2}x", sim.summary(), speed);
        }
        if sim.time == time_limit {
            panic!(
                "Time limit {} hit, but some expectations never met: {:?}",
                sim.time, expectations
            );
        }
    }
}
