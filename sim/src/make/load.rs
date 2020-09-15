use crate::{AlertHandler, Scenario, ScenarioModifier, Sim, SimOptions};
use abstutil::CmdArgs;
use map_model::{Map, MapEdits};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::Deserialize;

const RNG_SEED: u8 = 42;

#[derive(Clone, Deserialize)]
pub struct SimFlags {
    pub load: String,
    pub modifiers: Vec<ScenarioModifier>,
    #[serde(skip_deserializing)]
    pub rng_seed: u8,
    #[serde(skip_deserializing)]
    pub opts: SimOptions,
}

impl SimFlags {
    pub fn from_args(args: &mut CmdArgs) -> SimFlags {
        let rng_seed = args
            .optional_parse("--rng_seed", |s| s.parse())
            .unwrap_or(RNG_SEED);

        SimFlags {
            load: args
                .optional_free()
                .unwrap_or_else(|| abstutil::path_map("montlake")),
            modifiers: Vec::new(),
            rng_seed,
            opts: SimOptions {
                run_name: args
                    .optional("--run_name")
                    .unwrap_or_else(|| "unnamed".to_string()),
                use_freeform_policy_everywhere: args.enabled("--freeform_policy"),
                dont_block_the_box: !args.enabled("--disable_block_the_box"),
                recalc_lanechanging: !args.enabled("--disable_recalc_lc"),
                break_turn_conflict_cycles: !args.enabled("--disable_break_turn_conflict_cycles"),
                handle_uber_turns: !args.enabled("--disable_handle_uber_turns"),
                enable_pandemic_model: if args.enabled("--pandemic") {
                    Some(XorShiftRng::from_seed([rng_seed; 16]))
                } else {
                    None
                },
                alerts: args
                    .optional("--alerts")
                    .map(|x| match x.as_ref() {
                        "print" => AlertHandler::Print,
                        "block" => AlertHandler::Block,
                        "silence" => AlertHandler::Silence,
                        _ => panic!("Bad --alerts={}. Must be print|block|silence", x),
                    })
                    .unwrap_or(AlertHandler::Print),
                pathfinding_upfront: args.enabled("--pathfinding_upfront"),
                live_map_edits: args.enabled("--live_map_edits"),
                infinite_parking: args.enabled("--infinite_parking"),
            },
        }
    }

    // TODO rename seattle_test
    pub fn for_test(run_name: &str) -> SimFlags {
        SimFlags::synthetic_test("montlake", run_name)
    }

    pub fn synthetic_test(map: &str, run_name: &str) -> SimFlags {
        SimFlags {
            load: abstutil::path_map(map),
            modifiers: Vec::new(),
            rng_seed: RNG_SEED,
            opts: SimOptions::new(run_name),
        }
    }

    pub fn make_rng(&self) -> XorShiftRng {
        XorShiftRng::from_seed([self.rng_seed; 16])
    }

    // Convenience method to setup everything.
    pub fn load(&self, timer: &mut abstutil::Timer) -> (Map, Sim, XorShiftRng) {
        let mut rng = self.make_rng();

        let mut opts = self.opts.clone();

        if self.load.starts_with(&abstutil::path("player/saves/")) {
            timer.note(format!("Resuming from {}", self.load));

            let mut sim: Sim = abstutil::read_binary(self.load.clone(), timer);

            let mut map = Map::new(abstutil::path_map(&sim.map_name), timer);
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
                    // Little brittle. Sometimes legitimate edits wind up being saved without a
                    // proper name.
                    if sim.edits_name.starts_with("Untitled Proposal") {
                        warn!(
                            "Sim savestate refers to edits \"{}\", but not using them: {}",
                            sim.edits_name, err
                        );
                    } else {
                        panic!("Couldn't load edits \"{}\": {}", sim.edits_name, err);
                    }
                }
            }
            sim.restore_paths(&map, timer);

            (map, sim, rng)
        } else if self.load.starts_with(&abstutil::path("system/scenarios/")) {
            timer.note(format!(
                "Seeding the simulation from scenario {}",
                self.load
            ));

            let mut scenario: Scenario = abstutil::read_binary(self.load.clone(), timer);

            let map = Map::new(abstutil::path_map(&scenario.map_name), timer);

            let mut modifier_rng = self.make_rng();
            for m in &self.modifiers {
                scenario = m.apply(&map, scenario, &mut modifier_rng);
            }

            if opts.run_name == "unnamed" {
                opts.run_name = scenario.scenario_name.clone();
            }
            let mut sim = Sim::new(&map, opts, timer);
            scenario.instantiate(&mut sim, &map, &mut rng, timer);

            (map, sim, rng)
        } else if self.load.starts_with(&abstutil::path_all_raw_maps())
            || self.load.starts_with(&abstutil::path_all_synthetic_maps())
            || self.load.starts_with(&abstutil::path_all_maps())
        {
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
