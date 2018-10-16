use ezgui::{Canvas, GfxCtx, LogScroller, UserInput, Wizard, WrappedWizard};
use map_model::Map;
use objects::ID;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::{choose_edits, choose_scenario, load_ab_test, Colorizer};
use sim::{ABTest, SimFlags};
use ui::PerMapUI;

pub enum ABTestManager {
    Inactive,
    PickABTest(Wizard),
    ManageABTest(ABTest, LogScroller),
}

impl ABTestManager {
    pub fn new() -> ABTestManager {
        ABTestManager::Inactive
    }

    // May return a new primary and secondary UI
    pub fn event(
        &mut self,
        input: &mut UserInput,
        selected: Option<ID>,
        map: &Map,
        kml: &Option<String>,
        current_flags: &SimFlags,
    ) -> (bool, Option<(PerMapUI, PerMapUI)>) {
        let mut new_ui: Option<(PerMapUI, PerMapUI)> = None;
        let mut new_state: Option<ABTestManager> = None;
        match self {
            ABTestManager::Inactive => {
                if selected.is_none()
                    && input.unimportant_key_pressed(Key::B, SIM_SETUP, "manage A/B tests")
                {
                    new_state = Some(ABTestManager::PickABTest(Wizard::new()));
                }
            }
            ABTestManager::PickABTest(ref mut wizard) => {
                if let Some(ab_test) = pick_ab_test(map, wizard.wrap(input)) {
                    let scroller = LogScroller::new_from_lines(ab_test.describe());
                    new_state = Some(ABTestManager::ManageABTest(ab_test, scroller));
                } else if wizard.aborted() {
                    new_state = Some(ABTestManager::Inactive);
                }
            }
            ABTestManager::ManageABTest(test, ref mut scroller) => {
                if input.key_pressed(Key::R, "run this A/B test") {
                    new_ui = Some(launch_test(test, kml, current_flags));
                    new_state = Some(ABTestManager::Inactive);
                }
                if scroller.event(input) {
                    new_state = Some(ABTestManager::Inactive);
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        let active = match self {
            ABTestManager::Inactive => false,
            _ => true,
        };
        (active, new_ui)
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        match self {
            ABTestManager::Inactive => {}
            ABTestManager::PickABTest(wizard) => {
                wizard.draw(g, canvas);
            }
            ABTestManager::ManageABTest(_, scroller) => {
                scroller.draw(g, canvas);
            }
        }
    }
}

impl Colorizer for ABTestManager {}

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
    kml: &Option<String>,
    current_flags: &SimFlags,
) -> (PerMapUI, PerMapUI) {
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
        kml,
    );
    let secondary = PerMapUI::new(
        SimFlags {
            load,
            rng_seed,
            run_name: format!("{} with {}", test.test_name, test.edits2_name),
            edits_name: test.edits2_name.clone(),
        },
        kml,
    );
    // That's all! The scenario will be instantiated.
    (primary, secondary)
}
