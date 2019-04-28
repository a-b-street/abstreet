use crate::abtest::{ABTestMode, State};
use crate::game::{GameState, Mode};
use crate::plugins::{choose_edits, choose_scenario, load_ab_test};
use crate::state::{Flags, PerMapUI, UIState};
use ezgui::{EventCtx, GfxCtx, LogScroller, Wizard, WrappedWizard};
use map_model::Map;
use sim::{ABTest, SimFlags};
use std::path::PathBuf;

pub enum ABTestSetup {
    Pick(Wizard),
    Manage(ABTest, LogScroller),
}

impl ABTestSetup {
    pub fn event(state: &mut GameState, ctx: &mut EventCtx) {
        match state.mode {
            Mode::ABTest(ref mut mode) => match mode.state {
                State::Setup(ref mut setup) => match setup {
                    ABTestSetup::Pick(ref mut wizard) => {
                        if let Some(ab_test) = pick_ab_test(
                            &state.ui.state.primary.map,
                            wizard.wrap(ctx.input, ctx.canvas),
                        ) {
                            let scroller =
                                LogScroller::new(ab_test.test_name.clone(), ab_test.describe());
                            *setup = ABTestSetup::Manage(ab_test, scroller);
                        } else if wizard.aborted() {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        }
                    }
                    ABTestSetup::Manage(test, ref mut scroller) => {
                        ctx.input.set_mode_with_prompt(
                            "A/B Test Editor",
                            format!("A/B Test Editor for {}", test.test_name),
                            &ctx.canvas,
                        );
                        if scroller.event(ctx.input) {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        } else if ctx.input.modal_action("run A/B test") {
                            state.mode = launch_test(test, &mut state.ui.state, ctx);
                        }
                    }
                },
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            ABTestSetup::Pick(wizard) => {
                wizard.draw(g);
            }
            ABTestSetup::Manage(_, scroller) => {
                scroller.draw(g);
            }
        }
    }
}

fn pick_ab_test(map: &Map, mut wizard: WrappedWizard) -> Option<ABTest> {
    let load_existing = "Load existing A/B test";
    let create_new = "Create new A/B test";
    if wizard.choose_string("What A/B test to manage?", vec![load_existing, create_new])?
        == load_existing
    {
        load_ab_test(map, &mut wizard, "Load which A/B test?")
    } else {
        let test_name = wizard.input_string("Name the A/B test")?;
        let ab_test = ABTest {
            test_name,
            map_name: map.get_name().to_string(),
            scenario_name: choose_scenario(map, &mut wizard, "What scenario to run?")?,
            edits1_name: choose_edits(map, &mut wizard, "For the 1st run, what map edits to use?")?,
            edits2_name: choose_edits(map, &mut wizard, "For the 2nd run, what map edits to use?")?,
        };
        ab_test.save();
        Some(ab_test)
    }
}

fn launch_test(test: &ABTest, state: &mut UIState, ctx: &mut EventCtx) -> Mode {
    println!("Launching A/B test {}...", test.test_name);
    let load = PathBuf::from(format!(
        "../data/scenarios/{}/{}.json",
        test.map_name, test.scenario_name
    ));
    let current_flags = &state.primary.current_flags;
    let rng_seed = if current_flags.sim_flags.rng_seed.is_some() {
        current_flags.sim_flags.rng_seed
    } else {
        Some(42)
    };

    // TODO Cheaper to load the edits for the map and then instantiate the scenario for the
    // primary.
    let (primary, _) = PerMapUI::new(
        Flags {
            sim_flags: SimFlags {
                load: load.clone(),
                rng_seed,
                run_name: format!("{} with {}", test.test_name, test.edits1_name),
                edits_name: test.edits1_name.clone(),
            },
            ..current_flags.clone()
        },
        &state.cs,
        ctx.prerender,
    );
    let (secondary, _) = PerMapUI::new(
        Flags {
            sim_flags: SimFlags {
                load,
                rng_seed,
                run_name: format!("{} with {}", test.test_name, test.edits2_name),
                edits_name: test.edits2_name.clone(),
            },
            ..current_flags.clone()
        },
        &state.cs,
        ctx.prerender,
    );

    state.primary = primary;
    Mode::ABTest(ABTestMode {
        desired_speed: 1.0,
        state: State::Paused,
        secondary: Some(secondary),
        diff_trip: None,
        diff_all: None,
    })
}
