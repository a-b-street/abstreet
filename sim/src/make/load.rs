use anyhow::Result;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use structopt::StructOpt;

use abstio::MapName;
use map_model::{Map, MapEdits};
use synthpop::{Scenario, ScenarioModifier};

use crate::{Sim, SimOptions};

/// SimFlags specifies a simulation to setup. After parsing from structopt, you must call
/// `initialize`.
#[derive(Clone, StructOpt)]
pub struct SimFlags {
    /// A path to some file:
    ///
    /// - some kind of map: start an empty simulation on the map
    /// - a scenario
    /// - a savestate: restore the simulation exactly from some savestate
    #[structopt()]
    load_path: Option<String>,
    /// The same as `load_path`, but with a default value filled out. Call `initialize` to set this
    /// up.
    #[structopt(skip)]
    pub load: String,
    /// A JSON list of modifiers to transform the scenario. These can be generated with the GUI.
    #[structopt(long, parse(try_from_str = parse_modifiers), default_value = "[]")]
    pub scenario_modifiers: ModifierList,
    /// An arbitrary number to seed the random number generator. This is input to the deterministic
    /// simulation, so different values affect results.
    // TODO default_value can only handle strings, so copying SimFlags::RNG_SEED
    #[structopt(long, default_value = "42")]
    pub rng_seed: u64,
    #[structopt(flatten)]
    pub opts: SimOptions,
}

// See https://github.com/TeXitoi/structopt/issues/94
type ModifierList = Vec<ScenarioModifier>;

fn parse_modifiers(x: &str) -> Result<ModifierList> {
    abstutil::from_json(&x.to_string().into_bytes())
}

impl SimFlags {
    pub const RNG_SEED: u64 = 42;

    pub fn initialize(&mut self) {
        // default_value can't call functions and this value can't be hardcoded
        self.load = self
            .load_path
            .clone()
            .unwrap_or_else(|| MapName::seattle("montlake").path());
    }

    // TODO rename seattle_test
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags {
            load_path: None,
            load: MapName::seattle("montlake").path(),
            scenario_modifiers: Vec::new(),
            rng_seed: SimFlags::RNG_SEED,
            opts: SimOptions::new(run_name),
        }
    }

    pub fn make_rng(&self) -> XorShiftRng {
        XorShiftRng::seed_from_u64(self.rng_seed)
    }

    /// Loads a map and simulation. Not appropriate for use in the UI or on web.
    pub fn load_synchronously(&self, timer: &mut abstutil::Timer) -> (Map, Sim, XorShiftRng) {
        if self.load.is_empty() {
            panic!("You forgot to call initialize on SimFlags after parsing from structopt");
        }

        let mut rng = self.make_rng();

        let mut opts = self.opts.clone();

        if self.load.starts_with(&abstio::path_player("saves/")) {
            info!("Resuming from {}", self.load);

            let sim: Sim = abstio::must_read_object(self.load.clone(), timer);

            let mut map = Map::load_synchronously(sim.map_name.path(), timer);
            match MapEdits::load_from_file(
                &map,
                abstio::path_edits(map.get_name(), &sim.edits_name),
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
            info!("Seeding the simulation from scenario {}", self.load);

            let mut scenario: Scenario = abstio::must_read_object(self.load.clone(), timer);

            let map = Map::load_synchronously(scenario.map_name.path(), timer);

            for m in &self.scenario_modifiers {
                scenario = m.apply(&map, scenario, &mut rng);
            }

            if opts.run_name == "unnamed" {
                opts.run_name = scenario.scenario_name.clone();
            }
            let mut sim = Sim::new(&map, opts);
            sim.instantiate(&scenario, &map, &mut rng, timer);

            (map, sim, rng)
        } else if self.load.contains("/raw_maps/") || self.load.contains("/maps/") {
            info!("Loading map {}", self.load);

            let map = Map::load_synchronously(self.load.clone(), timer);

            timer.start("create sim");
            let sim = Sim::new(&map, opts);
            timer.stop("create sim");

            (map, sim, rng)
        } else {
            panic!("Don't know how to load {}", self.load);
        }
    }
}
