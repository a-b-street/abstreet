use abstutil;
use control::ControlMap;
use map_model::Map;
use {MapEdits, Scenario, Sim, Tick};

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
    // TODO rename seattle_test
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags::synthetic_test("montlake", run_name)
    }

    pub fn synthetic_test(map: &str, run_name: &str) -> SimFlags {
        SimFlags {
            load: format!("../data/raw_maps/{}.abst", map),
            rng_seed: Some(42),
            run_name: run_name.to_string(),
            edits_name: "no_edits".to_string(),
        }
    }
}

// Convenience method to setup everything.
pub fn load(
    flags: SimFlags,
    savestate_every: Option<Tick>,
    timer: &mut abstutil::Timer,
) -> (Map, ControlMap, Sim) {
    if flags.load.contains("data/save/") {
        assert_eq!(flags.edits_name, "no_edits");

        info!("Resuming from {}", flags.load);
        timer.start("read sim savestate");
        let sim: Sim = abstutil::read_json(&flags.load).expect("loading sim state failed");
        timer.stop("read sim savestate");

        let edits: MapEdits = if sim.edits_name == "no_edits" {
            MapEdits::new()
        } else {
            abstutil::read_json(&format!(
                "../data/edits/{}/{}.json",
                sim.map_name, sim.edits_name
            )).unwrap()
        };

        // Try loading the pre-baked map first
        let map: Map = abstutil::read_binary(
            &format!("../data/maps/{}_{}.abst", sim.map_name, sim.edits_name),
            timer,
        ).unwrap_or_else(|_| {
            let map_path = format!("../data/raw_maps/{}.abst", sim.map_name);
            Map::new(&map_path, edits.road_edits.clone(), timer)
                .expect(&format!("Couldn't load map from {}", map_path))
        });
        let control_map = ControlMap::new(&map, edits.stop_signs, edits.traffic_signals);

        (map, control_map, sim)
    } else if flags.load.contains("data/scenarios/") {
        info!("Seeding the simulation from scenario {}", flags.load);
        let scenario: Scenario = abstutil::read_json(&flags.load).expect("loading scenario failed");
        let edits = load_edits(&scenario.map_name, &flags);

        // Try loading the pre-baked map first
        let map: Map = abstutil::read_binary(
            &format!(
                "../data/maps/{}_{}.abst",
                scenario.map_name, edits.edits_name
            ),
            timer,
        ).unwrap_or_else(|_| {
            let map_path = format!("../data/raw_maps/{}.abst", scenario.map_name);
            Map::new(&map_path, edits.road_edits.clone(), timer)
                .expect(&format!("Couldn't load map from {}", map_path))
        });
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
    } else if flags.load.contains("data/raw_maps/") {
        // TODO relative dir is brittle; match more cautiously
        let map_name = flags
            .load
            .trim_left_matches("../data/raw_maps/")
            .trim_right_matches(".abst")
            .to_string();
        info!("Loading map {}", flags.load);
        let edits = load_edits(&map_name, &flags);
        let map = Map::new(&flags.load, edits.road_edits.clone(), timer)
            .expect(&format!("Couldn't load map from {}", flags.load));
        let control_map = ControlMap::new(&map, edits.stop_signs, edits.traffic_signals);
        timer.start("create sim");
        let sim = Sim::new(&map, flags.run_name, flags.rng_seed, savestate_every);
        timer.stop("create sim");
        (map, control_map, sim)
    } else if flags.load.contains("data/maps/") {
        assert_eq!(flags.edits_name, "no_edits");

        info!("Loading map {}", flags.load);
        let map: Map = abstutil::read_binary(&flags.load, timer)
            .expect(&format!("Couldn't load map from {}", flags.load));
        // TODO Bit sad to load edits to reconstitute ControlMap, but this is necessary right now
        let edits: MapEdits = abstutil::read_json(&format!(
            "../data/edits/{}/{}.json",
            map.get_name(),
            map.get_road_edits().edits_name
        )).unwrap();
        let control_map = ControlMap::new(&map, edits.stop_signs, edits.traffic_signals);
        timer.start("create sim");
        let sim = Sim::new(&map, flags.run_name, flags.rng_seed, savestate_every);
        timer.stop("create sim");
        (map, control_map, sim)
    } else {
        panic!("Don't know how to load {}", flags.load);
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
