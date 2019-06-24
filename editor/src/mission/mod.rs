mod all_trips;
mod dataviz;
mod individ_trips;
mod neighborhood;
mod scenario;
mod trips;

use self::trips::{pick_time_range, trips_to_scenario};
use crate::game::{State, Transition};
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, ModalMenu, Wizard, WrappedWizard};
use geom::Duration;
use map_model::Map;
use sim::Scenario;

pub struct MissionEditMode {
    menu: ModalMenu,
}

impl MissionEditMode {
    pub fn new(ctx: &EventCtx, ui: &mut UI) -> MissionEditMode {
        // TODO Warn first?
        ui.primary.reset_sim();

        MissionEditMode {
            menu: ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    vec![
                        (hotkey(Key::D), "visualize population data"),
                        (hotkey(Key::T), "visualize individual PSRC trips"),
                        (hotkey(Key::A), "visualize all PSRC trips"),
                    ],
                    vec![
                        (hotkey(Key::S), "set up simulation with PSRC trips"),
                        (hotkey(Key::Q), "create scenario from PSRC trips"),
                        (hotkey(Key::N), "manage neighborhoods"),
                        (hotkey(Key::W), "load scenario"),
                        (None, "create new scenario"),
                    ],
                    vec![(hotkey(Key::Escape), "quit")],
                ],
                ctx,
            ),
        }
    }
}

impl State for MissionEditMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return Transition::Pop;
        } else if self.menu.action("visualize population data") {
            return Transition::Push(Box::new(dataviz::DataVisualizer::new(ctx, ui)));
        } else if self.menu.action("visualize individual PSRC trips") {
            return Transition::Push(Box::new(individ_trips::TripsVisualizer::new(ctx, ui)));
        } else if self.menu.action("visualize all PSRC trips") {
            return Transition::Push(Box::new(all_trips::TripsVisualizer::new(ctx, ui)));
        } else if self.menu.action("set up simulation with PSRC trips") {
            let scenario = trips_to_scenario(
                ctx,
                ui,
                Duration::ZERO,
                Duration::parse("23:59:59.9").unwrap(),
            );
            ctx.loading_screen("instantiate scenario", |_, timer| {
                scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary
                    .sim
                    .step(&ui.primary.map, Duration::const_seconds(0.1));
            });
            return Transition::Replace(Box::new(SandboxMode::new(ctx)));
        } else if self.menu.action("create scenario from PSRC trips") {
            return Transition::Push(Box::new(TripsToScenario {
                wizard: Wizard::new(),
            }));
        } else if self.menu.action("manage neighborhoods") {
            return Transition::Push(Box::new(neighborhood::NeighborhoodPicker::new()));
        } else if self.menu.action("load scenario") {
            return Transition::Push(Box::new(LoadScenario {
                wizard: Wizard::new(),
            }));
        } else if self.menu.action("create new scenario") {
            return Transition::Push(Box::new(CreateNewScenario {
                wizard: Wizard::new(),
            }));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}

struct TripsToScenario {
    wizard: Wizard,
}

impl State for TripsToScenario {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some((t1, t2)) = pick_time_range(self.wizard.wrap(ctx)) {
            trips_to_scenario(ctx, ui, t1, t2).save();
            return Transition::Pop;
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

struct LoadScenario {
    wizard: Wizard,
}

impl State for LoadScenario {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some(scenario) = load_scenario(&ui.primary.map, &mut self.wizard.wrap(ctx)) {
            return Transition::Replace(Box::new(scenario::ScenarioManager::new(scenario, ctx)));
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

struct CreateNewScenario {
    wizard: Wizard,
}

impl State for CreateNewScenario {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let mut wrapped = self.wizard.wrap(ctx);
        if let Some(name) = wrapped.input_string("Name the scenario") {
            return Transition::Replace(Box::new(scenario::ScenarioManager::new(
                Scenario {
                    scenario_name: name,
                    map_name: ui.primary.map.get_name().to_string(),
                    seed_parked_cars: Vec::new(),
                    spawn_over_time: Vec::new(),
                    border_spawn_over_time: Vec::new(),
                    individ_trips: Vec::new(),
                },
                ctx,
            )));
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

pub fn input_time(wizard: &mut WrappedWizard, query: &str) -> Option<Duration> {
    wizard.input_something(query, None, Box::new(|line| Duration::parse(&line)))
}

fn load_scenario(map: &Map, wizard: &mut WrappedWizard) -> Option<Scenario> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            "Load which scenario?",
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        )
        .map(|(_, s)| {
            abstutil::read_binary(
                &format!("../data/scenarios/{}/{}.bin", map.get_name(), s),
                &mut Timer::throwaway(),
            )
            .unwrap()
        })
}
