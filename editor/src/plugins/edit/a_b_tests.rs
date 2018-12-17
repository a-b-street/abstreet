use crate::objects::Ctx;
use crate::plugins::{choose_edits, choose_scenario, load_ab_test, Plugin, PluginCtx};
use crate::state::{PerMapUI, PluginsPerMap};
use ezgui::{Canvas, GfxCtx, Key, LogScroller, Wizard, WrappedWizard};
use map_model::Map;
use sim::{ABTest, SimFlags};

pub enum ABTestManager {
    PickABTest(Wizard),
    ManageABTest(ABTest, LogScroller),
}

impl ABTestManager {
    pub fn new(ctx: &mut PluginCtx) -> Option<ABTestManager> {
        if ctx.primary.current_selection.is_none() && ctx.input.action_chosen("manage A/B tests") {
            return Some(ABTestManager::PickABTest(Wizard::new()));
        }
        None
    }
}

impl Plugin for ABTestManager {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            ABTestManager::PickABTest(ref mut wizard) => {
                if let Some(ab_test) =
                    pick_ab_test(&ctx.primary.map, wizard.wrap(ctx.input, ctx.canvas))
                {
                    let scroller = LogScroller::new_from_lines(ab_test.describe());
                    *self = ABTestManager::ManageABTest(ab_test, scroller);
                } else if wizard.aborted() {
                    return false;
                }
            }
            ABTestManager::ManageABTest(test, ref mut scroller) => {
                if ctx.input.key_pressed(Key::R, "run this A/B test") {
                    let ((new_primary, new_primary_plugins), new_secondary) =
                        launch_test(test, &ctx.primary.current_flags, &ctx.canvas);
                    *ctx.primary = new_primary;
                    if let Some(p_plugins) = ctx.primary_plugins.as_mut() {
                        **p_plugins = new_primary_plugins;
                    }
                    *ctx.secondary = Some(new_secondary);
                    return false;
                }
                if scroller.event(ctx.input) {
                    return false;
                }
            }
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self {
            ABTestManager::PickABTest(wizard) => {
                wizard.draw(g, ctx.canvas);
            }
            ABTestManager::ManageABTest(_, scroller) => {
                scroller.draw(g, ctx.canvas);
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

fn launch_test(
    test: &ABTest,
    current_flags: &SimFlags,
    canvas: &Canvas,
) -> ((PerMapUI, PluginsPerMap), (PerMapUI, PluginsPerMap)) {
    info!("Launching A/B test {}...", test.test_name);
    let load = format!(
        "../data/scenarios/{}/{}.json",
        test.map_name, test.scenario_name
    );
    let rng_seed = if current_flags.rng_seed.is_some() {
        current_flags.rng_seed
    } else {
        Some(42)
    };

    let primary = PerMapUI::new(
        SimFlags {
            load: load.clone(),
            rng_seed,
            run_name: format!("{} with {}", test.test_name, test.edits1_name),
            edits_name: test.edits1_name.clone(),
        },
        None,
        canvas,
    );
    let secondary = PerMapUI::new(
        SimFlags {
            load,
            rng_seed,
            run_name: format!("{} with {}", test.test_name, test.edits2_name),
            edits_name: test.edits2_name.clone(),
        },
        None,
        canvas,
    );
    // That's all! The scenario will be instantiated.
    (primary, secondary)
}
