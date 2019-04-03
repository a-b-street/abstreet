use crate::objects::DrawCtx;
use crate::plugins::{
    choose_intersection, choose_neighborhood, choose_origin_destination, input_time,
    input_weighted_usize, load_scenario, BlockingPlugin, PluginCtx,
};
use abstutil::Timer;
use ezgui::{Color, Drawable, GfxCtx, LogScroller, Wizard, WrappedWizard};
use geom::{Distance, Duration, Line, Pt2D};
use map_model::{Map, Neighborhood};
use sim::{BorderSpawnOverTime, OriginDestination, Scenario, SeedParkedCars, SpawnOverTime};
use std::collections::BTreeMap;

pub enum ScenarioManager {
    PickScenario(Wizard),
    ManageScenario(Scenario, LogScroller),
    EditScenario(Scenario, Wizard),
    VisualizeScenario(Scenario, Drawable, BTreeMap<String, Region>),
}

impl ScenarioManager {
    pub fn new(ctx: &mut PluginCtx) -> Option<ScenarioManager> {
        if ctx.input.action_chosen("manage scenarios") {
            return Some(ScenarioManager::PickScenario(Wizard::new()));
        }
        None
    }
}

impl BlockingPlugin for ScenarioManager {
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
                    scenario.instantiate(
                        &mut ctx.primary.sim,
                        &ctx.primary.map,
                        &mut ctx.primary.current_flags.sim_flags.make_rng(),
                        &mut Timer::new("instantiate scenario"),
                    );
                    return false;
                } else if ctx.input.modal_action("visualize") {
                    let neighborhoods = Neighborhood::load_all(
                        ctx.primary.map.get_name(),
                        &ctx.primary.map.get_gps_bounds(),
                    );
                    let draw_all = ctx.prerender.upload_borrowed(
                        neighborhoods
                            .iter()
                            .enumerate()
                            .map(|(idx, (_, n))| (COLORS[idx % COLORS.len()], &n.polygon))
                            .collect::<Vec<_>>(),
                    );
                    let mapping = neighborhoods
                        .into_iter()
                        .enumerate()
                        .map(|(idx, (name, n))| {
                            (
                                name.clone(),
                                Region {
                                    name,
                                    color: COLORS[idx % COLORS.len()],
                                    center: n.polygon.center(),
                                },
                            )
                        })
                        .collect();
                    *self = ScenarioManager::VisualizeScenario(scenario.clone(), draw_all, mapping);
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
            ScenarioManager::VisualizeScenario(ref scenario, _, _) => {
                ctx.input.set_mode_with_prompt(
                    "Scenario Editor",
                    format!("Scenario Editor for {}", scenario.scenario_name),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("quit") {
                    return false;
                }
            }
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        match self {
            ScenarioManager::PickScenario(wizard) => {
                wizard.draw(g);
            }
            ScenarioManager::ManageScenario(_, scroller) => {
                scroller.draw(g);
            }
            ScenarioManager::EditScenario(_, wizard) => {
                if let Some(neighborhood) = wizard.current_menu_choice::<Neighborhood>() {
                    g.draw_polygon(ctx.cs.get("neighborhood polygon"), &neighborhood.polygon);
                }
                wizard.draw(g);
            }
            ScenarioManager::VisualizeScenario(ref scenario, ref draw_all, ref mapping) => {
                g.redraw(draw_all);

                // Aggregate by (src, dst) pair, breakdown over time and mode, etc.
                for s in &scenario.spawn_over_time {
                    // TODO Draw text label in neighborhood, then src is left and dst is right
                    let src = mapping[&s.start_from_neighborhood]
                        .center
                        .offset(-50.0, 0.0);
                    let dst = match s.goal {
                        OriginDestination::Neighborhood(ref n) => mapping[n].center,
                        OriginDestination::Border(i) => ctx.map.get_i(i).point,
                    }
                    .offset(50.0, 0.0);
                    // TODO Draw a self-loop or something
                    if src == dst {
                        continue;
                    }
                    g.draw_arrow(
                        // Source color, sure
                        mapping[&s.start_from_neighborhood].color.alpha(0.5),
                        // TODO Vary by (relative) number of agents
                        Distance::meters(100.0),
                        &Line::new(src, dst),
                    );
                }
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
    let randomize = "Randomly spawn stuff from/to every neighborhood";
    match wizard
        .choose_string(
            "What kind of edit?",
            vec![seed_parked, spawn, spawn_border, randomize],
        )?
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
                start_time: input_time(&mut wizard, "Start spawning when?")?,
                // TODO input interval, or otherwise enforce stop_time > start_time
                stop_time: input_time(&mut wizard, "Stop spawning when?")?,
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
                start_time: input_time(&mut wizard, "Start spawning when?")?,
                // TODO input interval, or otherwise enforce stop_time > start_time
                stop_time: input_time(&mut wizard, "Stop spawning when?")?,
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
        x if x == randomize => {
            let neighborhoods = Neighborhood::load_all(map.get_name(), &map.get_gps_bounds());
            for (src, _) in &neighborhoods {
                for (dst, _) in &neighborhoods {
                    scenario.spawn_over_time.push(SpawnOverTime {
                        num_agents: 100,
                        start_time: Duration::ZERO,
                        stop_time: Duration::minutes(10),
                        start_from_neighborhood: src.to_string(),
                        goal: OriginDestination::Neighborhood(dst.to_string()),
                        percent_biking: 0.1,
                        percent_use_transit: 0.2,
                    });
                }
            }
        }
        _ => unreachable!(),
    };
    Some(())
}

const COLORS: [Color; 3] = [
    // TODO these are awful choices
    Color::RED.alpha(0.8),
    Color::GREEN.alpha(0.8),
    Color::BLUE.alpha(0.8),
];

// Er, the info on top of Neighbohood
pub struct Region {
    name: String,
    color: Color,
    center: Pt2D,
}
