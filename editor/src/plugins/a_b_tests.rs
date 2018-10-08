use abstutil;
use ezgui::{Canvas, GfxCtx, LogScroller, UserInput, Wizard, WrappedWizard};
use map_model::Map;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::Colorizer;
use sim::{ABTest, MapEdits, Scenario};

pub enum ABTestManager {
    Inactive,
    PickABTest(Wizard),
    ManageABTest(ABTest, LogScroller),
}

impl ABTestManager {
    pub fn new() -> ABTestManager {
        ABTestManager::Inactive
    }

    pub fn event(&mut self, input: &mut UserInput, map: &Map) -> bool {
        let mut new_state: Option<ABTestManager> = None;
        match self {
            ABTestManager::Inactive => {
                if input.unimportant_key_pressed(Key::A, SIM_SETUP, "manage A/B tests") {
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
            ABTestManager::ManageABTest(_, ref mut scroller) => {
                // TODO Some key to run the test
                if scroller.event(input) {
                    new_state = Some(ABTestManager::Inactive);
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            ABTestManager::Inactive => false,
            _ => true,
        }
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
        let map_name = map.get_name().to_string();
        wizard
            .choose_something::<ABTest>(
                "Load which A/B test?",
                Box::new(move || abstutil::load_all_objects("ab_tests", &map_name)),
            ).map(|(_, t)| t)
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

// TODO it'd be neat to instead register parsers and choice generators on a wizard, then call them?
// these file-loading ones are especially boilerplate. maybe even just refactor that in the editor
// crate.

fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<Scenario>(
            query,
            Box::new(move || abstutil::load_all_objects("scenarios", &map_name)),
        ).map(|(n, _)| n)
}

fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<MapEdits>(
            query,
            Box::new(move || abstutil::load_all_objects("edits", &map_name)),
        ).map(|(n, _)| n)
}
