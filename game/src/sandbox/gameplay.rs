use crate::game::{Transition, WizardState};
use crate::sandbox::{analytics, bus_explorer, spawner, SandboxMode};
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, Line, ModalMenu, Text, Wizard};
use geom::Duration;
use map_model::BusRouteID;
use sim::Scenario;

#[derive(Clone)]
pub enum GameplayMode {
    // TODO Maybe this should be "sandbox"
    Freeform,
    PlayScenario(String),
    // Route name
    OptimizeBus(String),
}

pub struct GameplayState {
    pub mode: GameplayMode,
    pub menu: ModalMenu,
    state: State,
}

enum State {
    // TODO Maybe this one could remember what things were spawned, offer to replay this later
    Freeform,
    PlayScenario,
    OptimizeBus {
        route: BusRouteID,
        time: Duration,
        show_analytics: bool,
    },
}

impl GameplayState {
    pub fn initialize(mode: GameplayMode, ui: &mut UI, ctx: &mut EventCtx) -> GameplayState {
        let (state, maybe_scenario) = match mode.clone() {
            GameplayMode::Freeform => (
                GameplayState {
                    mode,
                    menu: ModalMenu::new(
                        "Freeform mode",
                        vec![(hotkey(Key::S), "start a scenario")],
                        ctx,
                    )
                    .disable_standalone_layout(),
                    state: State::Freeform,
                },
                None,
            ),
            GameplayMode::PlayScenario(scenario) => (
                GameplayState {
                    mode,
                    menu: ModalMenu::new(
                        &format!("Playing {}", scenario),
                        vec![(hotkey(Key::S), "start another scenario")],
                        ctx,
                    )
                    .disable_standalone_layout(),
                    state: State::PlayScenario,
                },
                Some(scenario),
            ),
            GameplayMode::OptimizeBus(route_name) => {
                let route = ui.primary.map.get_bus_route(&route_name).unwrap();
                (
                    GameplayState {
                        mode,
                        menu: ModalMenu::new(
                            &format!("Optimize {}", route_name),
                            vec![(hotkey(Key::E), "show bus route")],
                            ctx,
                        )
                        .disable_standalone_layout(),
                        state: State::OptimizeBus {
                            route: route.id,
                            time: Duration::ZERO,
                            show_analytics: false,
                        },
                    },
                    Some("weekday_typical_traffic_from_psrc".to_string()),
                )
            }
        };
        if let Some(scenario_name) = maybe_scenario {
            ctx.loading_screen("instantiate scenario", |_, timer| {
                let num_agents = ui.primary.current_flags.num_agents;
                let builtin = if let Some(n) = num_agents {
                    format!("random scenario with {} agents", n)
                } else {
                    "random scenario with some agents".to_string()
                };
                let scenario = if scenario_name == builtin {
                    if let Some(n) = num_agents {
                        Scenario::scaled_run(&ui.primary.map, n)
                    } else {
                        Scenario::small_run(&ui.primary.map)
                    }
                } else if scenario_name == "just buses" {
                    Scenario::empty(&ui.primary.map)
                } else {
                    abstutil::read_binary(
                        &abstutil::path1_bin(
                            &ui.primary.map.get_name(),
                            abstutil::SCENARIOS,
                            &scenario_name,
                        ),
                        timer,
                    )
                    .unwrap()
                };
                scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
            });
        }
        state
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        analytics: &mut analytics::Analytics,
    ) -> Option<Transition> {
        match self.state {
            State::Freeform => {
                self.menu.event(ctx);
                if self.menu.action("start a scenario") {
                    return Some(Transition::Push(WizardState::new(Box::new(
                        change_scenario,
                    ))));
                }
                if let Some(new_state) = spawner::AgentSpawner::new(ctx, ui) {
                    return Some(Transition::Push(new_state));
                }
            }
            State::PlayScenario => {
                self.menu.event(ctx);
                if self.menu.action("start another scenario") {
                    return Some(Transition::Push(WizardState::new(Box::new(
                        change_scenario,
                    ))));
                }
            }
            State::OptimizeBus {
                route,
                ref mut time,
                ref mut show_analytics,
            } => {
                // Something else might've changed analytics.
                if *show_analytics {
                    match analytics {
                        analytics::Analytics::BusRoute(_) => {}
                        _ => {
                            *show_analytics = false;
                            self.menu
                                .change_action("hide bus route", "show bus route", ctx);
                        }
                    }
                }

                // TODO Expensive
                if *time != ui.primary.sim.time() {
                    *time = ui.primary.sim.time();
                    self.menu.set_info(ctx, bus_route_panel(route, ui));
                    if *show_analytics {
                        *analytics = analytics::Analytics::BusRoute(
                            bus_explorer::ShowBusRoute::new(ui.primary.map.get_br(route), ui, ctx),
                        );
                    }
                }

                self.menu.event(ctx);
                if !*show_analytics
                    && self
                        .menu
                        .swap_action("show bus route", "hide bus route", ctx)
                {
                    *analytics = analytics::Analytics::BusRoute(bus_explorer::ShowBusRoute::new(
                        ui.primary.map.get_br(route),
                        ui,
                        ctx,
                    ));
                    *show_analytics = true;
                } else if *show_analytics
                    && self
                        .menu
                        .swap_action("hide bus route", "show bus route", ctx)
                {
                    *analytics = analytics::Analytics::Inactive;
                    *show_analytics = false;
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }
}

fn bus_route_panel(id: BusRouteID, ui: &UI) -> Text {
    let route = ui.primary.map.get_br(id);
    let arrivals = &ui.primary.sim.get_analytics().bus_arrivals;
    let mut txt = Text::new();
    for (idx, stop) in route.stops.iter().enumerate() {
        let prev = if idx == 0 { route.stops.len() } else { idx };
        let this = idx + 1;

        txt.add(Line(format!("Stop {}->{}: ", prev, this)));
        if let Some(ref times) = arrivals.get(&(*stop, route.id)) {
            txt.append(Line(format!(
                "{} ago",
                (ui.primary.sim.time() - *times.last().unwrap()).minimal_tostring()
            )));
        } else {
            txt.append(Line("no arrivals yet"));
        }
    }
    txt
}

fn change_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let num_agents = ui.primary.current_flags.num_agents;
    let builtin = if let Some(n) = num_agents {
        format!("random scenario with {} agents", n)
    } else {
        "random scenario with some agents".to_string()
    };
    let scenario_name = wiz
        .wrap(ctx)
        .choose_string("Instantiate which scenario?", || {
            let mut list =
                abstutil::list_all_objects(abstutil::SCENARIOS, ui.primary.map.get_name());
            list.push(builtin.clone());
            list.push("just buses".to_string());
            list
        })?;
    Some(Transition::Replace(Box::new(SandboxMode::new(
        ctx,
        ui,
        GameplayMode::PlayScenario(scenario_name),
    ))))
}
