use crate::abtest::{ABTestMode, State};
use crate::game::{GameState, Mode};
use crate::ui::{Flags, PerMapUI, UI};
use ezgui::{hotkey, EventCtx, GfxCtx, Key, LogScroller, ModalMenu, Wizard, WrappedWizard};
use map_model::Map;
use sim::{ABTest, SimFlags};
use std::path::PathBuf;

pub enum ABTestSetup {
    Pick(Wizard),
    Manage(ModalMenu, ABTest, LogScroller),
}

impl ABTestSetup {
    pub fn event(state: &mut GameState, ctx: &mut EventCtx) {
        match state.mode {
            Mode::ABTest(ref mut mode) => match mode.state {
                State::Setup(ref mut setup) => match setup {
                    ABTestSetup::Pick(ref mut wizard) => {
                        if let Some(ab_test) = pick_ab_test(&state.ui.primary.map, wizard.wrap(ctx))
                        {
                            let scroller =
                                LogScroller::new(ab_test.test_name.clone(), ab_test.describe());
                            *setup = ABTestSetup::Manage(
                                ModalMenu::new(
                                    &format!("A/B Test Editor for {}", ab_test.test_name),
                                    vec![
                                        (hotkey(Key::Escape), "quit"),
                                        (hotkey(Key::R), "run A/B test"),
                                    ],
                                    ctx,
                                ),
                                ab_test,
                                scroller,
                            );
                        } else if wizard.aborted() {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        }
                    }
                    ABTestSetup::Manage(ref mut menu, test, ref mut scroller) => {
                        ctx.canvas.handle_event(ctx.input);
                        menu.handle_event(ctx, None);
                        if scroller.event(ctx.input) {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        } else if menu.action("run A/B test") {
                            state.mode = launch_test(test, &mut state.ui, ctx);
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
            ABTestSetup::Manage(ref menu, _, scroller) => {
                scroller.draw(g);
                menu.draw(g);
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

fn launch_test(test: &ABTest, ui: &mut UI, ctx: &mut EventCtx) -> Mode {
    println!("Launching A/B test {}...", test.test_name);
    let load = PathBuf::from(format!(
        "../data/scenarios/{}/{}",
        test.map_name, test.scenario_name
    ));
    let current_flags = &ui.primary.current_flags;
    let rng_seed = if current_flags.sim_flags.rng_seed.is_some() {
        current_flags.sim_flags.rng_seed
    } else {
        Some(42)
    };

    // TODO Cheaper to load the edits for the map and then instantiate the scenario for the
    // primary.
    let primary = PerMapUI::new(
        Flags {
            sim_flags: SimFlags {
                load: load.clone(),
                rng_seed,
                run_name: Some(format!("{} with {}", test.test_name, test.edits1_name)),
                edits_name: test.edits1_name.clone(),
            },
            ..current_flags.clone()
        },
        &ui.cs,
        ctx,
    );
    let secondary = PerMapUI::new(
        Flags {
            sim_flags: SimFlags {
                load,
                rng_seed,
                run_name: Some(format!("{} with {}", test.test_name, test.edits2_name)),
                edits_name: test.edits2_name.clone(),
            },
            ..current_flags.clone()
        },
        &ui.cs,
        ctx,
    );

    ui.primary = primary;
    let mut mode = ABTestMode::new(ctx, ui);
    mode.state = State::Playing;
    mode.secondary = Some(secondary);
    Mode::ABTest(mode)
}

fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        )
        .map(|(n, _)| n)
}

fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || {
                let mut list = abstutil::list_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), "no_edits".to_string()));
                list
            }),
        )
        .map(|(n, _)| n)
}

fn load_ab_test(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<ABTest> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<ABTest>(
            query,
            Box::new(move || abstutil::load_all_objects("ab_tests", &map_name)),
        )
        .map(|(_, t)| t)
}
