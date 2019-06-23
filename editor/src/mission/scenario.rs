use crate::game::{State, Transition};
use crate::mission::input_time;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::WeightedUsizeChoice;
use ezgui::{
    hotkey, EventCtx, EventLoopMode, GfxCtx, Key, LogScroller, ModalMenu, Wizard, WrappedWizard,
};
use geom::Duration;
use map_model::{IntersectionID, Map, Neighborhood};
use sim::{BorderSpawnOverTime, OriginDestination, Scenario, SeedParkedCars, SpawnOverTime};

pub struct ScenarioManager {
    menu: ModalMenu,
    scenario: Scenario,
    scroller: LogScroller,
}

impl ScenarioManager {
    pub fn new(scenario: Scenario, ctx: &mut EventCtx) -> ScenarioManager {
        let scroller = LogScroller::new(scenario.scenario_name.clone(), scenario.describe());
        ScenarioManager {
            menu: ModalMenu::new(
                &format!("Scenario Editor for {}", scenario.scenario_name),
                vec![
                    (hotkey(Key::Escape), "quit"),
                    (hotkey(Key::S), "save"),
                    (hotkey(Key::E), "edit"),
                    (hotkey(Key::I), "instantiate"),
                ],
                ctx,
            ),
            scenario,
            scroller,
        }
    }
}

impl State for ScenarioManager {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);
        if self.menu.action("save") {
            self.scenario.save();
        } else if self.menu.action("edit") {
            return (
                Transition::Push(Box::new(ScenarioEditor {
                    scenario: self.scenario.clone(),
                    wizard: Wizard::new(),
                })),
                EventLoopMode::InputOnly,
            );
        } else if self.menu.action("instantiate") {
            ctx.loading_screen("instantiate scenario", |_, timer| {
                self.scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
            });
            return (
                Transition::Replace(Box::new(SandboxMode::new(ctx))),
                EventLoopMode::InputOnly,
            );
        } else if self.scroller.event(&mut ctx.input) {
            return (Transition::Pop, EventLoopMode::InputOnly);
        }
        (Transition::Keep, EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.scroller.draw(g);
        self.menu.draw(g);
    }
}

struct ScenarioEditor {
    scenario: Scenario,
    wizard: Wizard,
}

impl State for ScenarioEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        if let Some(()) = edit_scenario(&ui.primary.map, &mut self.scenario, self.wizard.wrap(ctx))
        {
            // TODO autosave, or at least make it clear there are unsaved edits
            let scenario = self.scenario.clone();
            return (
                Transition::PopWithData(Box::new(|state| {
                    let mut manager = state.downcast_mut::<ScenarioManager>().unwrap();
                    manager.scroller =
                        LogScroller::new(scenario.scenario_name.clone(), scenario.describe());
                    manager.scenario = scenario;
                })),
                EventLoopMode::InputOnly,
            );
        } else if self.wizard.aborted() {
            return (Transition::Pop, EventLoopMode::InputOnly);
        }
        (Transition::Keep, EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(neighborhood) = self.wizard.current_menu_choice::<Neighborhood>() {
            g.draw_polygon(ui.cs.get("neighborhood polygon"), &neighborhood.polygon);
        }
        self.wizard.draw(g);
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

fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    let gps_bounds = map.get_gps_bounds().clone();
    // Load the full object, since we usually visualize the neighborhood when menuing over it
    wizard
        .choose_something_no_keys::<Neighborhood>(
            query,
            Box::new(move || Neighborhood::load_all(&map_name, &gps_bounds)),
        )
        .map(|(n, _)| n)
}

fn input_weighted_usize(wizard: &mut WrappedWizard, query: &str) -> Option<WeightedUsizeChoice> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| WeightedUsizeChoice::parse(&line)),
    )
}

// TODO Validate the intersection exists? Let them pick it with the cursor?
fn choose_intersection(wizard: &mut WrappedWizard, query: &str) -> Option<IntersectionID> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| usize::from_str_radix(&line, 10).ok().map(IntersectionID)),
    )
}

fn choose_origin_destination(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<OriginDestination> {
    let neighborhood = "Neighborhood";
    let border = "Border intersection";
    if wizard.choose_string(query, vec![neighborhood, border])? == neighborhood {
        choose_neighborhood(map, wizard, query).map(OriginDestination::Neighborhood)
    } else {
        choose_intersection(wizard, query).map(OriginDestination::Border)
    }
}
