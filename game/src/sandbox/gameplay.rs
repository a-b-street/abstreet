use crate::game::{Transition, WizardState};
use crate::sandbox::{analytics, bus_explorer, spawner, SandboxMode};
use crate::ui::UI;
use ezgui::{hotkey, Choice, EventCtx, GfxCtx, Key, Line, ModalMenu, Text, Wizard};
use geom::{Duration, DurationHistogram, Statistic};
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
        stat: Statistic,
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
                        vec![
                            (hotkey(Key::S), "start a scenario"),
                            (hotkey(Key::H), "help"),
                        ],
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
                        vec![
                            (hotkey(Key::S), "start another scenario"),
                            (hotkey(Key::H), "help"),
                        ],
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
                            vec![
                                (hotkey(Key::E), "show bus route"),
                                (hotkey(Key::S), "change statistic"),
                                (hotkey(Key::H), "help"),
                            ],
                            ctx,
                        )
                        .disable_standalone_layout(),
                        state: State::OptimizeBus {
                            route: route.id,
                            time: Duration::ZERO,
                            show_analytics: false,
                            stat: Statistic::Max,
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
                if self.menu.action("help") {
                    return Some(help(vec!["This simulation is empty by default.", "Try right-clicking an intersection and choosing to spawn agents (or just hover over it and press Z).", "You can also spawn agents from buildings or lanes.", "You can also start a full scenario to get realistic traffic."]));
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
                if self.menu.action("help") {
                    return Some(help(vec![
                        "Do things seem a bit quiet?",
                        "The simulation starts at midnight, so you might need to wait a bit.",
                        "Try using the speed controls on the left.",
                    ]));
                }
            }
            State::OptimizeBus {
                route,
                ref mut time,
                ref mut show_analytics,
                ref mut stat,
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
                    self.menu.set_info(ctx, bus_route_panel(route, ui, *stat));
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

                if self.menu.action("change statistic") {
                    return Some(Transition::Push(WizardState::new(Box::new(
                        move |wiz, ctx, _| {
                            // TODO Filter out existing. Make this kind of thing much easier.
                            let (_, new_stat) = wiz.wrap(ctx).choose(
                                "Show which statistic on frequency a bus stop is visited?",
                                || {
                                    Statistic::all()
                                        .into_iter()
                                        .map(|s| Choice::new(s.to_string(), s))
                                        .collect()
                                },
                            )?;
                            Some(Transition::PopWithData(Box::new(move |state, _, _| {
                                let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                                match sandbox.gameplay.state {
                                    State::OptimizeBus {
                                        ref mut stat,
                                        ref mut time,
                                        ..
                                    } => {
                                        // Force recalculation
                                        *time = Duration::ZERO;
                                        *stat = new_stat;
                                    }
                                    _ => unreachable!(),
                                }
                            })))
                        },
                    ))));
                }
                if self.menu.action("help") {
                    return Some(help(vec![
                        "First find where the bus gets stuck.",
                        "Then use edit mode to try to speed things up.",
                        "Try making dedicated bus lanes",
                        "and adjusting traffic signals.",
                    ]));
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }
}

fn bus_route_panel(id: BusRouteID, ui: &UI, stat: Statistic) -> Text {
    let route = ui.primary.map.get_br(id);
    let arrivals = &ui.primary.sim.get_analytics().bus_arrivals;
    let mut txt = Text::new();
    txt.add(Line(format!("{} frequency stop is visited", stat)));
    for (idx, stop) in route.stops.iter().enumerate() {
        txt.add(Line(format!("Stop {}: ", idx + 1)));
        if let Some(ref times) = arrivals.get(&(*stop, route.id)) {
            if times.len() < 2 {
                txt.append(Line("only one arrival so far"));
            } else {
                let mut distrib: DurationHistogram = Default::default();
                for pair in times.windows(2) {
                    distrib.add(pair[1] - pair[0]);
                }
                txt.append(Line(distrib.select(stat).minimal_tostring()));
            }
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

// TODO Word wrap
fn help(lines: Vec<&'static str>) -> Transition {
    Transition::Push(WizardState::new(Box::new(move |wiz, ctx, _| {
        if wiz.wrap(ctx).acknowledge("Help", || lines.clone()) {
            Some(Transition::Pop)
        } else {
            None
        }
    })))
}
