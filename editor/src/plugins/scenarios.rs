use abstutil;
use ezgui::{Canvas, GfxCtx, UserInput, LogScroller};
use map_model::Map;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::Colorizer;
use sim::{SeedParkedCars, Scenario, SpawnOverTime};
use wizard::{Wizard, WrappedWizard};

pub enum ScenarioManager {
    Inactive,
    PickScenario(Wizard),
    EditScenario(Scenario, LogScroller),
}

impl ScenarioManager {
    pub fn new() -> ScenarioManager {
        ScenarioManager::Inactive
    }

    pub fn event(&mut self, input: &mut UserInput, map: &Map) -> bool {
        let mut new_state: Option<ScenarioManager> = None;
        match self {
            ScenarioManager::Inactive => {
                if input.unimportant_key_pressed(
                    Key::W,
                    SIM_SETUP,
                    "manage scenarios",
                ) {
                    new_state = Some(ScenarioManager::PickScenario(Wizard::new()));
                }
            }
            ScenarioManager::PickScenario(ref mut wizard) => {
                if let Some(scenario) = pick_scenario(wizard.wrap(input, map)) {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    new_state = Some(ScenarioManager::EditScenario(scenario, scroller));
                } else if wizard.aborted() {
                    new_state = Some(ScenarioManager::Inactive);
                }
            }
            ScenarioManager::EditScenario(_, ref mut scroller) => {
                if scroller.event(input) {
                    new_state = Some(ScenarioManager::Inactive);
                }
                // TODO edit it
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            ScenarioManager::Inactive => false,
            _ => true,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        match self {
            ScenarioManager::Inactive => {}
            ScenarioManager::PickScenario(wizard) => {
                wizard.draw(g, canvas);
            }
            ScenarioManager::EditScenario(_, scroller) => {
                scroller.draw(g, canvas);
            }
        }
    }
}

impl Colorizer for ScenarioManager {}

fn pick_scenario(mut wizard: WrappedWizard) -> Option<Scenario> {
    let load_existing = "Load existing scenario";
    let create_new = "Create new scenario";

    if wizard.choose("What scenario to edit?", vec![load_existing, create_new])? == load_existing {
        // TODO Constantly load them?! Urgh...
        let scenarios: Vec<(String, Scenario)> = abstutil::load_all_objects("scenarios", wizard.map.get_name());
        let name = wizard.choose("Load which scenario?", scenarios.iter().map(|(n, _)| n.as_str()).collect())?;
        // TODO But we want to store the associated data in the wizard and get it out!
        Some(scenarios.into_iter().find(|(n, _)| name == *n).map(|(_, s)| s).unwrap())
    } else {
        let scenario_name = wizard.input_string("Name the scenario")?;
        Some(Scenario {
            scenario_name,
            map_name: wizard.map.get_name().to_string(),
            seed_parked_cars: Vec::new(),
            spawn_over_time: Vec::new(),
        })
    }
}

fn workflow(mut wizard: WrappedWizard) -> Option<SpawnOverTime> {
    Some(SpawnOverTime {
        num_agents: wizard.input_usize("Spawn how many agents?")?,
        start_tick: wizard.input_tick("Start spawning when?")?,
        // TODO input interval, or otherwise enforce stop_tick > start_tick
        stop_tick: wizard.input_tick("Stop spawning when?")?,
        percent_drive: wizard.input_percent("What percent should drive?")?,
        start_from_neighborhood: wizard.choose_neighborhood("Where should the agents start?")?,
        go_to_neighborhood: wizard.choose_neighborhood("Where should the agents go?")?,
    })
}

fn workflow2(mut wizard: WrappedWizard) -> Option<SeedParkedCars> {
    Some(SeedParkedCars {
        neighborhood: wizard.choose_neighborhood("Seed parked cars in what area?")?,
        percent_to_fill: wizard.input_percent("What percent of parking spots to populate?")?,
    })
}
