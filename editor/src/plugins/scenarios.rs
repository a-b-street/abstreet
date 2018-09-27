use abstutil;
use ezgui::{Canvas, GfxCtx, LogScroller, UserInput, Wizard, WrappedWizard};
use geom::Polygon;
use map_model::Map;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::Colorizer;
use sim::{Neighborhood, Scenario, SeedParkedCars, SpawnOverTime, Tick, Sim};

pub enum ScenarioManager {
    Inactive,
    PickScenario(Wizard),
    ManageScenario(Scenario, LogScroller),
    EditScenario(Scenario, Wizard),
}

impl ScenarioManager {
    pub fn new() -> ScenarioManager {
        ScenarioManager::Inactive
    }

    pub fn event(&mut self, input: &mut UserInput, map: &Map, sim: &mut Sim) -> bool {
        let mut new_state: Option<ScenarioManager> = None;
        match self {
            ScenarioManager::Inactive => {
                if input.unimportant_key_pressed(Key::W, SIM_SETUP, "manage scenarios") {
                    new_state = Some(ScenarioManager::PickScenario(Wizard::new()));
                }
            }
            ScenarioManager::PickScenario(ref mut wizard) => {
                if let Some(scenario) = pick_scenario(map, wizard.wrap(input)) {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    new_state = Some(ScenarioManager::ManageScenario(scenario, scroller));
                } else if wizard.aborted() {
                    new_state = Some(ScenarioManager::Inactive);
                }
            }
            ScenarioManager::ManageScenario(scenario, ref mut scroller) => {
                // TODO Keys on top of the scroller? Weird...
                // TODO Would use S for save, except sim controls always runs... maybe it shouldnt'
                // do that after all.
                if input.key_pressed(Key::Q, "save this scenario") {
                    let path = format!(
                        "../data/scenarios/{}/{}",
                        scenario.map_name, scenario.scenario_name
                    );
                    abstutil::write_json(&path, scenario).expect("Saving scenario failed");
                    info!("Saved {}", path);
                } else if input.key_pressed(Key::E, "edit this scenario") {
                    new_state = Some(ScenarioManager::EditScenario(
                        scenario.clone(),
                        Wizard::new(),
                    ));
                } else if input.key_pressed(Key::I, "instantiate this scenario") {
                    scenario.instantiate(sim);
                } else if scroller.event(input) {
                    new_state = Some(ScenarioManager::Inactive);
                }
            }
            ScenarioManager::EditScenario(ref mut scenario, ref mut wizard) => {
                if let Some(()) = edit_scenario(map, scenario, wizard.wrap(input)) {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    // TODO autosave, or at least make it clear there are unsaved edits
                    new_state = Some(ScenarioManager::ManageScenario(scenario.clone(), scroller));
                } else if wizard.aborted() {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    new_state = Some(ScenarioManager::ManageScenario(scenario.clone(), scroller));
                }
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
            ScenarioManager::ManageScenario(_, scroller) => {
                scroller.draw(g, canvas);
            }
            ScenarioManager::EditScenario(_, wizard) => {
                if let Some(neighborhood) = wizard.current_menu_choice::<Neighborhood>() {
                    g.draw_polygon([0.0, 0.0, 1.0, 0.6], &Polygon::new(&neighborhood.points));
                }
                wizard.draw(g, canvas);
            }
        }
    }
}

impl Colorizer for ScenarioManager {}

fn pick_scenario(map: &Map, mut wizard: WrappedWizard) -> Option<Scenario> {
    let load_existing = "Load existing scenario";
    let create_new = "Create new scenario";
    if wizard.choose_string("What scenario to edit?", vec![load_existing, create_new])?
        == load_existing
    {
        let map_name = map.get_name().to_string();
        wizard
            .choose_something::<Scenario>(
                "Load which scenario?",
                Box::new(move || abstutil::load_all_objects("scenarios", &map_name)),
            ).map(|(_, s)| s)
    } else {
        let scenario_name = wizard.input_string("Name the scenario")?;
        Some(Scenario {
            scenario_name,
            map_name: map.get_name().to_string(),
            seed_parked_cars: Vec::new(),
            spawn_over_time: Vec::new(),
        })
    }
}

fn edit_scenario(map: &Map, scenario: &mut Scenario, mut wizard: WrappedWizard) -> Option<()> {
    let seed_parked = "Seed parked cars";
    let spawn = "Spawn agents";
    if wizard.choose_string("What kind of edit?", vec![seed_parked, spawn])? == seed_parked {
        scenario.seed_parked_cars.push(SeedParkedCars {
            neighborhood: choose_neighborhood(map, &mut wizard, "Seed parked cars in what area?")?,
            percent_to_fill: wizard.input_percent("What percent of parking spots to populate?")?,
        });
        Some(())
    } else {
        scenario.spawn_over_time.push(SpawnOverTime {
            num_agents: wizard.input_usize("Spawn how many agents?")?,
            start_tick: input_tick(&mut wizard, "Start spawning when?")?,
            // TODO input interval, or otherwise enforce stop_tick > start_tick
            stop_tick: input_tick(&mut wizard, "Stop spawning when?")?,
            percent_drive: wizard.input_percent("What percent should drive?")?,
            start_from_neighborhood: choose_neighborhood(
                map,
                &mut wizard,
                "Where should the agents start?",
            )?,
            go_to_neighborhood: choose_neighborhood(
                map,
                &mut wizard,
                "Where should the agents go?",
            )?,
        });
        Some(())
    }
}

// TODO it'd be neat to instead register parsers and choice generators on a wizard, then call them?

fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<Neighborhood>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        ).map(|(n, _)| n)
}

fn input_tick(wizard: &mut WrappedWizard, query: &str) -> Option<Tick> {
    wizard.input_something(query, Box::new(|line| Tick::parse(&line)))
}
