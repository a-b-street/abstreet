use ezgui::{Color, GfxCtx, LogScroller, Wizard, WrappedWizard};
use map_model::Map;
use objects::{Ctx, SIM_SETUP};
use piston::input::Key;
use plugins::{
    choose_intersection, choose_neighborhood, choose_origin_destination, input_tick,
    input_weighted_usize, load_scenario, Plugin, PluginCtx,
};
use sim::{BorderSpawnOverTime, Neighborhood, Scenario, SeedParkedCars, SpawnOverTime};

pub enum ScenarioManager {
    PickScenario(Wizard),
    ManageScenario(Scenario, LogScroller),
    EditScenario(Scenario, Wizard),
}

impl ScenarioManager {
    pub fn new(ctx: &mut PluginCtx) -> Option<ScenarioManager> {
        if ctx
            .input
            .unimportant_key_pressed(Key::W, SIM_SETUP, "manage scenarios")
        {
            return Some(ScenarioManager::PickScenario(Wizard::new()));
        }
        None
    }
}

impl Plugin for ScenarioManager {
    fn new_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let (input, map, sim) = (&mut ctx.input, &ctx.primary.map, &mut ctx.primary.sim);

        let mut new_state: Option<ScenarioManager> = None;
        match self {
            ScenarioManager::PickScenario(ref mut wizard) => {
                if let Some(scenario) = pick_scenario(map, wizard.wrap(input)) {
                    let scroller = LogScroller::new_from_lines(scenario.describe());
                    new_state = Some(ScenarioManager::ManageScenario(scenario, scroller));
                } else if wizard.aborted() {
                    return false;
                }
            }
            ScenarioManager::ManageScenario(scenario, ref mut scroller) => {
                // TODO Keys on top of the scroller? Weird...
                // TODO Would use S for save, except sim controls always runs... maybe it shouldnt'
                // do that after all.
                if input.key_pressed(Key::Q, "save this scenario") {
                    scenario.save();
                } else if input.key_pressed(Key::E, "edit this scenario") {
                    new_state = Some(ScenarioManager::EditScenario(
                        scenario.clone(),
                        Wizard::new(),
                    ));
                } else if input.key_pressed(Key::I, "instantiate this scenario") {
                    scenario.instantiate(sim, map);
                    return false;
                } else if scroller.event(input) {
                    return false;
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
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        match self {
            ScenarioManager::PickScenario(wizard) => {
                wizard.draw(g, ctx.canvas);
            }
            ScenarioManager::ManageScenario(_, scroller) => {
                scroller.draw(g, ctx.canvas);
            }
            ScenarioManager::EditScenario(_, wizard) => {
                if let Some(neighborhood) = wizard.current_menu_choice::<Neighborhood>() {
                    g.draw_polygon(
                        ctx.cs
                            .get("neighborhood polygon", Color::rgba(0, 0, 255, 0.6)),
                        &neighborhood.polygon,
                    );
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
