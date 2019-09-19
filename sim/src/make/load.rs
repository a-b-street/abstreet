use crate::{Scenario, Sim, SimOptions};
use abstutil::CmdArgs;
use geom::Duration;
use map_model::{Map, MapEdits};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

#[derive(Clone)]
pub struct SimFlags {
    pub load: String,
    pub rng_seed: Option<u8>,
    pub run_name: Option<String>,
    pub savestate_every: Option<Duration>,
    pub freeform_policy: bool,
    pub disable_block_the_box: bool,
}

impl SimFlags {
    pub fn from_args(args: &mut CmdArgs) -> SimFlags {
        SimFlags {
            load: args
                .optional_free()
                .unwrap_or_else(|| "../data/maps/montlake.bin".to_string()),
            rng_seed: args
                .optional("--rng_seed")
                .map(|s| s.parse::<u8>().unwrap()),
            run_name: args.optional("--run_name"),
            savestate_every: args
                .optional("--savestate_every")
                .map(|s| Duration::parse(&s).unwrap()),
            freeform_policy: args.enabled("--freeform_policy"),
            disable_block_the_box: args.enabled("--disable_block_the_box"),
        }
    }

    // TODO rename seattle_test
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags::synthetic_test("montlake", run_name)
    }

    pub fn synthetic_test(map: &str, run_name: &str) -> SimFlags {
        SimFlags {
            load: abstutil::path_map(map),
            rng_seed: Some(42),
            run_name: Some(run_name.to_string()),
            savestate_every: None,
            freeform_policy: false,
            disable_block_the_box: false,
        }
    }

    pub fn make_rng(&self) -> XorShiftRng {
        if let Some(seed) = self.rng_seed {
            XorShiftRng::from_seed([seed; 16])
        } else {
            XorShiftRng::from_entropy()
        }
    }

    // Convenience method to setup everything.
    pub fn load(&self, timer: &mut abstutil::Timer) -> (Map, Sim, XorShiftRng) {
        let mut rng = self.make_rng();

        let mut opts = SimOptions {
            run_name: self
                .run_name
                .clone()
                .unwrap_or_else(|| "unnamed".to_string()),
            savestate_every: self.savestate_every,
            use_freeform_policy_everywhere: self.freeform_policy,
            disable_block_the_box: self.disable_block_the_box,
        };

        if self.load.starts_with("../data/save/") {
            timer.note(format!("Resuming from {}", self.load));

            let sim: Sim =
                abstutil::read_binary(&self.load, timer).expect("loading sim state failed");

            let mut map: Map =
                abstutil::read_binary(&abstutil::path_map(&sim.map_name), timer).unwrap();
            map.apply_edits(MapEdits::load(map.get_name(), &sim.edits_name), timer);
            map.recalculate_pathfinding_after_edits(timer);

            (map, sim, rng)
        } else if self.load.starts_with("../data/scenarios/") {
            timer.note(format!(
                "Seeding the simulation from scenario {}",
                self.load
            ));

            let scenario: Scenario =
                abstutil::read_binary(&self.load, timer).expect("loading scenario failed");

            let map: Map =
                abstutil::read_binary(&abstutil::path_map(&scenario.map_name), timer).unwrap();

            opts.run_name = self
                .run_name
                .clone()
                .unwrap_or_else(|| scenario.scenario_name.clone());
            let mut sim = Sim::new(&map, opts);
            scenario.instantiate(&mut sim, &map, &mut rng, timer);

            (map, sim, rng)
        } else if self.load.starts_with("../data/raw_maps/") {
            timer.note(format!("Loading map {}", self.load));

            let map = Map::new(&self.load, timer)
                .expect(&format!("Couldn't load map from {}", self.load));

            timer.start("create sim");
            let sim = Sim::new(&map, opts);
            timer.stop("create sim");

            (map, sim, rng)
        } else if self.load.starts_with("../data/maps/") {
            timer.note(format!("Loading map {}", self.load));

            let map: Map = abstutil::read_binary(&self.load, timer)
                .expect(&format!("Couldn't load map from {}", self.load));

            timer.start("create sim");
            let sim = Sim::new(&map, opts);
            timer.stop("create sim");

            (map, sim, rng)
        } else {
            panic!("Don't know how to load {}", self.load);
        }
    }
}
