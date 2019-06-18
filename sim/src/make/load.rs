use crate::{Scenario, Sim};
use abstutil;
use geom::Duration;
use map_model::{Map, MapEdits};
use rand::{FromEntropy, SeedableRng};
use rand_xorshift::XorShiftRng;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "sim_flags")]
pub struct SimFlags {
    /// Map, scenario, or savestate to load
    #[structopt(
        name = "load",
        parse(from_os_str),
        default_value = "../data/maps/montlake.bin"
    )]
    pub load: PathBuf,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    pub rng_seed: Option<u8>,

    /// Run name for savestating
    #[structopt(long = "run_name")]
    pub run_name: Option<String>,
}

impl SimFlags {
    // TODO rename seattle_test
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags::synthetic_test("montlake", run_name)
    }

    pub fn synthetic_test(map: &str, run_name: &str) -> SimFlags {
        SimFlags {
            load: PathBuf::from(format!("../data/maps/{}.bin", map)),
            rng_seed: Some(42),
            run_name: Some(run_name.to_string()),
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
    pub fn load(
        &self,
        savestate_every: Option<Duration>,
        timer: &mut abstutil::Timer,
    ) -> (Map, Sim, XorShiftRng) {
        let mut rng = self.make_rng();

        if self.load.starts_with(Path::new("../data/save/")) {
            timer.note(format!("Resuming from {}", self.load.display()));

            timer.start("read sim savestate");
            let sim: Sim =
                abstutil::read_json(self.load.to_str().unwrap()).expect("loading sim state failed");
            timer.stop("read sim savestate");

            let mut map: Map =
                abstutil::read_binary(&format!("../data/maps/{}.bin", sim.map_name), timer)
                    .unwrap();
            map.apply_edits(MapEdits::load(map.get_name(), &sim.edits_name), timer);
            map.recalculate_pathfinding_after_edits(timer);

            (map, sim, rng)
        } else if self.load.starts_with(Path::new("../data/scenarios/")) {
            timer.note(format!(
                "Seeding the simulation from scenario {}",
                self.load.display()
            ));

            let scenario: Scenario = abstutil::read_binary(self.load.to_str().unwrap(), timer)
                .expect("loading scenario failed");

            let map: Map =
                abstutil::read_binary(&format!("../data/maps/{}.bin", scenario.map_name), timer)
                    .unwrap();

            let mut sim = Sim::new(
                &map,
                self.run_name
                    .clone()
                    .unwrap_or_else(|| scenario.scenario_name.clone()),
                savestate_every,
            );
            scenario.instantiate(&mut sim, &map, &mut rng, timer);

            (map, sim, rng)
        } else if self.load.starts_with(Path::new("../data/raw_maps/")) {
            timer.note(format!("Loading map {}", self.load.display()));

            let map = Map::new(self.load.to_str().unwrap(), timer)
                .expect(&format!("Couldn't load map from {}", self.load.display()));

            timer.start("create sim");
            let sim = Sim::new(
                &map,
                self.run_name
                    .clone()
                    .unwrap_or_else(|| "unnamed".to_string()),
                savestate_every,
            );
            timer.stop("create sim");

            (map, sim, rng)
        } else if self.load.starts_with(Path::new("../data/maps/")) {
            timer.note(format!("Loading map {}", self.load.display()));

            let map: Map = abstutil::read_binary(self.load.to_str().unwrap(), timer)
                .expect(&format!("Couldn't load map from {}", self.load.display()));

            timer.start("create sim");
            let sim = Sim::new(
                &map,
                self.run_name
                    .clone()
                    .unwrap_or_else(|| "unnamed".to_string()),
                savestate_every,
            );
            timer.stop("create sim");

            (map, sim, rng)
        } else {
            panic!("Don't know how to load {}", self.load.display());
        }
    }
}
