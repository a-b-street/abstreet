use crate::{Scenario, Sim};
use abstutil;
use geom::Duration;
use map_model::{Map, MapEdits};
use rand::{FromEntropy, SeedableRng};
use rand_xorshift::XorShiftRng;
use structopt::StructOpt;

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
            load: format!("../data/maps/{}_no_edits.abst", map),
            rng_seed: Some(42),
            run_name: run_name.to_string(),
            edits_name: "no_edits".to_string(),
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

        if self.load.contains("data/save/") {
            assert_eq!(self.edits_name, "no_edits");

            timer.note(format!("Resuming from {}", self.load));
            timer.start("read sim savestate");
            let sim: Sim = abstutil::read_json(&self.load).expect("loading sim state failed");
            timer.stop("read sim savestate");

            let edits: MapEdits = if sim.edits_name == "no_edits" {
                MapEdits::new(&sim.map_name)
            } else {
                abstutil::read_json(&format!(
                    "../data/edits/{}/{}.json",
                    sim.map_name, sim.edits_name
                ))
                .unwrap()
            };

            // Try loading the pre-baked map first
            let map: Map = abstutil::read_binary(
                &format!("../data/maps/{}_{}.abst", sim.map_name, sim.edits_name),
                timer,
            )
            .unwrap_or_else(|_| {
                let map_path = format!("../data/raw_maps/{}.abst", sim.map_name);
                Map::new(&map_path, edits, timer)
                    .expect(&format!("Couldn't load map from {}", map_path))
            });

            (map, sim, rng)
        } else if self.load.contains("data/scenarios/") {
            timer.note(format!(
                "Seeding the simulation from scenario {}",
                self.load
            ));
            let scenario: Scenario =
                abstutil::read_json(&self.load).expect("loading scenario failed");
            let edits = self.load_edits(&scenario.map_name);

            // Try loading the pre-baked map first
            let map: Map = abstutil::read_binary(
                &format!(
                    "../data/maps/{}_{}.abst",
                    scenario.map_name, edits.edits_name
                ),
                timer,
            )
            .unwrap_or_else(|_| {
                let map_path = format!("../data/raw_maps/{}.abst", scenario.map_name);
                Map::new(&map_path, edits, timer)
                    .expect(&format!("Couldn't load map from {}", map_path))
            });
            let mut sim = Sim::new(
                &map,
                // TODO or the scenario name if no run name
                self.run_name.clone(),
                savestate_every,
            );
            scenario.instantiate(&mut sim, &map, &mut rng, timer);
            (map, sim, rng)
        } else if self.load.contains("data/raw_maps/") {
            // TODO relative dir is brittle; match more cautiously
            let map_name = self
                .load
                .trim_left_matches("../data/raw_maps/")
                .trim_right_matches(".abst")
                .to_string();
            timer.note(format!("Loading map {}", self.load));
            let edits = self.load_edits(&map_name);
            let map = Map::new(&self.load, edits, timer)
                .expect(&format!("Couldn't load map from {}", self.load));
            timer.start("create sim");
            let sim = Sim::new(&map, self.run_name.clone(), savestate_every);
            timer.stop("create sim");
            (map, sim, rng)
        } else if self.load.contains("data/maps/") {
            assert_eq!(self.edits_name, "no_edits");

            timer.note(format!("Loading map {}", self.load));
            let map: Map = abstutil::read_binary(&self.load, timer)
                .expect(&format!("Couldn't load map from {}", self.load));
            timer.start("create sim");
            let sim = Sim::new(&map, self.run_name.clone(), savestate_every);
            timer.stop("create sim");
            (map, sim, rng)
        } else {
            panic!("Don't know how to load {}", self.load);
        }
    }

    fn load_edits(&self, map_name: &str) -> MapEdits {
        if self.edits_name == "no_edits" {
            return MapEdits::new(map_name);
        }
        if self.edits_name.contains("data/") || self.edits_name.contains(".json") {
            panic!(
                "{} should just be a plain name, not a full path",
                self.edits_name
            );
        }
        let edits: MapEdits = abstutil::read_json(&format!(
            "../data/edits/{}/{}.json",
            map_name, self.edits_name
        ))
        .unwrap();
        edits
    }
}
