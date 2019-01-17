use crate::objects::Ctx;
use crate::plugins::{
    choose_intersection, choose_neighborhood, choose_origin_destination, input_tick,
    input_weighted_usize, load_scenario, Plugin, PluginCtx,
};
use ezgui::{GfxCtx, LogScroller, Wizard, WrappedWizard};
use map_model::{Map, Neighborhood};
use sim::{BorderSpawnOverTime, Scenario, SeedParkedCars, SpawnOverTime};

pub enum ScenarioManager {
    PickScenario(Wizard),
    ManageScenario(Scenario, LogScroller),
    EditScenario(Scenario, Wizard),
}

impl ScenarioManager {
    pub fn new(ctx: &mut PluginCtx) -> Option<ScenarioManager> {
        if ctx.input.action_chosen("manage scenarios") {
            return Some(ScenarioManager::PickScenario(Wizard::new()));
        }
        None
    }
}

impl Plugin for ScenarioManager {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            ScenarioManager::PickScenario(ref mut wizard) => {
                if let Some(scenario) =
                    pick_scenario(&ctx.primary.map, wizard.wrap(&mut ctx.input, ctx.canvas))
                {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    *self = ScenarioManager::ManageScenario(scenario, scroller);
                } else if wizard.aborted() {
                    return false;
                }
            }
            ScenarioManager::ManageScenario(scenario, ref mut scroller) => {
                ctx.input.set_mode_with_prompt(
                    "Scenario Editor",
                    format!("Scenario Editor for {}", scenario.scenario_name),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("save") {
                    scenario.save();
                } else if ctx.input.modal_action("edit") {
                    *self = ScenarioManager::EditScenario(scenario.clone(), Wizard::new());
                } else if ctx.input.modal_action("instantiate") {
                    scenario.instantiate(&mut ctx.primary.sim, &ctx.primary.map);
                    return false;
                } else if scroller.event(&mut ctx.input) {
                    return false;
                }
            }
            ScenarioManager::EditScenario(ref mut scenario, ref mut wizard) => {
                if let Some(()) = edit_scenario(
                    &ctx.primary.map,
                    scenario,
                    wizard.wrap(&mut ctx.input, ctx.canvas),
                ) {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    // TODO autosave, or at least make it clear there are unsaved edits
                    *self = ScenarioManager::ManageScenario(scenario.clone(), scroller);
                } else if wizard.aborted() {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    *self = ScenarioManager::ManageScenario(scenario.clone(), scroller);
                }
            }
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self {
            ScenarioManager::PickScenario(wizard) => {
                wizard.draw(g, ctx.canvas);
            }
            ScenarioManager::ManageScenario(_, scroller) => {
                scroller.draw(g, ctx.canvas);
            }
            ScenarioManager::EditScenario(_, wizard) => {
                if let Some(neighborhood) = wizard.current_menu_choice::<Neighborhood>() {
                    g.draw_polygon(ctx.cs.get("neighborhood polygon"), &neighborhood.polygon);
                }
                wizard.draw(g, ctx.canvas);
            }
        }
    }
}

fn pick_scenario(map: &Map, mut wizard: WrappedWizard) -> Option<Scenario> {
    let load_existing = "Load existing scenario";
    let create_new = "Create new scenario";
    if wizard.choose_string("What scenario to edit?", vec![load_existing, create_new])?
        == load_existing
    {
        load_scenario(map, &mut wizard, "Load which scenario?")
    } else {
        let scenario_name = wizard.input_string("Name the scenario")?;
        Some(Scenario {
            scenario_name,
            map_name: map.get_name().to_string(),
            seed_parked_cars: Vec::new(),
            spawn_over_time: Vec::new(),
            border_spawn_over_time: Vec::new(),
        })
    }
}

fn edit_scenario(map: &Map, scenario: &mut Scenario, mut wizard: WrappedWizard) -> Option<()> {
    let seed_parked = "Seed parked cars";
    let spawn = "Spawn agents";
    let spawn_border = "Spawn agents from a border";
    match wizard
        .choose_string("What kind of edit?", vec![seed_parked, spawn, spawn_border])?
        .as_str()
    {
        x if x == seed_parked => {
            scenario.seed_parked_cars.push(SeedParkedCars {
                neighborhood: choose_neighborhood(
                    map,
                    &mut wizard,
                    "Seed parked cars in what area?",
                )?,
                cars_per_building: input_weighted_usize(
                    &mut wizard,
                    "How many cars per building? (ex: 4,4,2)",
                )?,
            });
        }
        x if x == spawn => {
            scenario.spawn_over_time.push(SpawnOverTime {
                num_agents: wizard.input_usize("Spawn how many agents?")?,
                start_tick: input_tick(&mut wizard, "Start spawning when?")?,
                // TODO input interval, or otherwise enforce stop_tick > start_tick
                stop_tick: input_tick(&mut wizard, "Stop spawning when?")?,
                start_from_neighborhood: choose_neighborhood(
                    map,
                    &mut wizard,
                    "Where should the agents start?",
                )?,
                goal: choose_origin_destination(map, &mut wizard, "Where should the agents go?")?,
                percent_biking: wizard
                    .input_percent("What percent of the walking trips will bike instead?")?,
                percent_use_transit: wizard.input_percent(
                    "What percent of the walking trips will consider taking transit?",
                )?,
            });
        }
        x if x == spawn_border => {
            scenario.border_spawn_over_time.push(BorderSpawnOverTime {
                num_peds: wizard.input_usize("Spawn how many pedestrians?")?,
                num_cars: wizard.input_usize("Spawn how many cars?")?,
                num_bikes: wizard.input_usize("Spawn how many bikes?")?,
                start_tick: input_tick(&mut wizard, "Start spawning when?")?,
                // TODO input interval, or otherwise enforce stop_tick > start_tick
                stop_tick: input_tick(&mut wizard, "Stop spawning when?")?,
                // TODO validate it's a border!
                start_from_border: choose_intersection(
                    &mut wizard,
                    "Which border should the agents spawn at?",
                )?,
                goal: choose_origin_destination(map, &mut wizard, "Where should the agents go?")?,
                percent_use_transit: wizard.input_percent(
                    "What percent of the walking trips will consider taking transit?",
                )?,
            });
        }
        _ => unreachable!(),
    };
    Some(())
}
