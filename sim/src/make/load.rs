use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use abstutil::{CmdArgs, MapName};
use map_model::{Map, MapEdits};

use crate::{Scenario, ScenarioModifier, Sim, SimOptions};

/// SimFlags specifies a simulation to setup.
#[derive(Clone)]
pub struct SimFlags {
    /// A path to some file.
    /// - a savestate: restore the simulation exactly from some savestate
    /// - a scenario
    /// - some kind of map: start an empty simulation on the map
    pub load: String,
    pub modifiers: Vec<ScenarioModifier>,
    pub rng_seed: u64,
    pub opts: SimOptions,
}

impl SimFlags {
    pub const RNG_SEED: u64 = 42;

    pub fn from_args(args: &mut CmdArgs) -> SimFlags {
        let rng_seed = args
            .optional_parse("--rng_seed", |s| s.parse())
            .unwrap_or(SimFlags::RNG_SEED);
        let modifiers: Vec<ScenarioModifier> = args
            .optional_parse("--scenario_modifiers", |s| {
                abstutil::from_json(&s.to_string().into_bytes())
            })
            .unwrap_or_else(Vec::new);
        SimFlags {
            load: args
                .optional_free()
                .unwrap_or_else(|| MapName::seattle("montlake").path()),
            modifiers,
            rng_seed,
            opts: SimOptions::from_args(args, rng_seed),
        }
    }

    // TODO rename seattle_test
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags {
            load: MapName::seattle("montlake").path(),
            modifiers: Vec::new(),
            rng_seed: SimFlags::RNG_SEED,
            opts: SimOptions::new(run_name),
        }
    }

    pub fn make_rng(&self) -> XorShiftRng {
        XorShiftRng::seed_from_u64(self.rng_seed)
    }

    pub fn load(&self, timer: &mut abstutil::Timer) -> (Map, Sim, XorShiftRng) {
        let mut rng = self.make_rng();

        let mut opts = self.opts.clone();

        if self.load.starts_with(&abstutil::path("player/saves/")) {
            timer.note(format!("Resuming from {}", self.load));

            let sim: Sim = abstutil::must_read_object(self.load.clone(), timer);

            let mut map = Map::new(sim.map_name.path(), timer);
            match MapEdits::load(
                &map,
                abstutil::path_edits(map.get_name(), &sim.edits_name),
                timer,
            ) {
                Ok(edits) => {
                    map.must_apply_edits(edits, timer);
                    map.recalculate_pathfinding_after_edits(timer);
                }
                Err(err) => {
                    panic!("Couldn't load edits \"{}\": {}", sim.edits_name, err);
                }
            }

            (map, sim, rng)
        } else if self.load.contains("/scenarios/") {
            timer.note(format!(
                "Seeding the simulation from scenario {}",
                self.load
            ));

            let mut scenario: Scenario = abstutil::must_read_object(self.load.clone(), timer);

            let map = Map::new(scenario.map_name.path(), timer);

            for m in &self.modifiers {
                scenario = m.apply(&map, scenario);
            }

            if opts.run_name == "unnamed" {
                opts.run_name = scenario.scenario_name.clone();
            }
            let mut sim = Sim::new(&map, opts, timer);
            scenario.instantiate(&mut sim, &map, &mut rng, timer);

            (map, sim, rng)
        } else if self.load.contains("/raw_maps/") || self.load.contains("/maps/") {
            timer.note(format!("Loading map {}", self.load));

            let map = Map::new(self.load.clone(), timer);

            timer.start("create sim");
            let sim = Sim::new(&map, opts, timer);
            timer.stop("create sim");

            (map, sim, rng)
        } else {
            panic!("Don't know how to load {}", self.load);
        }
    }
}
