use abstutil;
use control::ControlMap;
use map_model::{Edits, LaneID, Map};
use std::iter;
use {Sim, Tick};

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

    sim.seed_bus(
        vec![
            map.get_l(LaneID(309)).bus_stops[0].clone(),
            map.get_l(LaneID(840)).bus_stops[0].clone(),
        ],
        map,
    );
}

pub fn big_spawn(sim: &mut Sim, map: &Map) {
    sim.seed_parked_cars(0.95);
    sim.seed_walking_trips(&map, 1000);
    sim.seed_driving_trips(&map, 1000);
}

// TODO share the helpers for spawning specific parking spots and stuff?

// TODO time limit?
pub fn run_until_done(
    sim: &mut Sim,
    map: &Map,
    control_map: &ControlMap,
    expectations: Vec<Box<Fn(&Sim) -> bool>>,
) {
    let mut benchmark = sim.start_benchmark();
    let mut expectations_met: Vec<bool> = iter::repeat(false).take(expectations.len()).collect();
    loop {
        sim.step(&map, &control_map);
        if sim.time.is_multiple_of(Tick::from_seconds(60)) {
            let speed = sim.measure_speed(&mut benchmark);
            println!("{0}, speed = {1:.2}x", sim.summary(), speed);
        }
        for (idx, e) in expectations.iter().enumerate() {
            if e(sim) {
                expectations_met[idx] = true;
            }
        }
        if sim.is_done() {
            break;
        }
    }
    let satisfied = expectations_met.into_iter().filter(|b| *b).count();
    if satisfied != expectations.len() {
        panic!(
            "Sim done at {}, but only satisfied {} of {} expectations",
            sim.time,
            satisfied,
            expectations.len()
        );
    }
}
