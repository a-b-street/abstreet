use crate::common::{Plot, Series};
use crate::game::{msg, Transition, WizardState};
use crate::helpers::rotating_color_total;
use crate::render::AgentColorScheme;
use crate::sandbox::overlays::Overlays;
use crate::sandbox::{bus_explorer, spawner, SandboxMode};
use crate::ui::UI;
use abstutil::{prettyprint_usize, Timer};
use ezgui::{
    hotkey, Choice, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text, TextSpan, Wizard,
};
use geom::{Duration, Statistic};
use map_model::BusRouteID;
use sim::{Analytics, Scenario, TripMode};

#[derive(Clone)]
pub enum GameplayMode {
    // TODO Maybe this should be "sandbox"
    Freeform,
    PlayScenario(String),
    // Route name
    OptimizeBus(String),
    CreateGridlock,
    // TODO Be able to filter population by more factors
    FasterTrips(TripMode),
}

pub struct GameplayState {
    pub mode: GameplayMode,
    pub menu: ModalMenu,
    state: State,
    prebaked: Analytics,
}

enum State {
    // TODO Maybe this one could remember what things were spawned, offer to replay this later
    Freeform,
    PlayScenario,
    OptimizeBus {
        route: BusRouteID,
        time: Duration,
        stat: Statistic,
    },
    CreateGridlock {
        time: Duration,
    },
    FasterTrips {
        mode: TripMode,
        time: Duration,
    },
}

impl GameplayState {
    pub fn initialize(mode: GameplayMode, ui: &mut UI, ctx: &mut EventCtx) -> GameplayState {
        let prebaked: Analytics = abstutil::read_binary(
            &abstutil::path_prebaked_results(ui.primary.map.get_name()),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| {
            println!("WARNING! No prebaked sim analytics. Only freeform mode will work.");
            Analytics::new()
        });

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
                    prebaked,
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
                    prebaked,
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
                                (hotkey(Key::T), "show delays over time"),
                                (hotkey(Key::S), "change statistic"),
                                (hotkey(Key::H), "help"),
                            ],
                            ctx,
                        )
                        .disable_standalone_layout(),
                        state: State::OptimizeBus {
                            route: route.id,
                            time: Duration::ZERO,
                            stat: Statistic::Max,
                        },
                        prebaked,
                    },
                    Some("weekday_typical_traffic_from_psrc".to_string()),
                )
            }
            GameplayMode::CreateGridlock => (
                GameplayState {
                    mode,
                    menu: ModalMenu::new(
                        "Cause gridlock",
                        vec![
                            (hotkey(Key::E), "show agent delay"),
                            (hotkey(Key::H), "help"),
                        ],
                        ctx,
                    )
                    .disable_standalone_layout(),
                    state: State::CreateGridlock {
                        time: Duration::ZERO,
                    },
                    prebaked,
                },
                Some("weekday_typical_traffic_from_psrc".to_string()),
            ),
            GameplayMode::FasterTrips(trip_mode) => (
                GameplayState {
                    mode,
                    menu: ModalMenu::new(
                        &format!("Speed up {} trips", trip_mode),
                        vec![(hotkey(Key::H), "help")],
                        ctx,
                    )
                    .disable_standalone_layout(),
                    state: State::FasterTrips {
                        mode: trip_mode,
                        time: Duration::ZERO,
                    },
                    prebaked,
                },
                Some("weekday_typical_traffic_from_psrc".to_string()),
            ),
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
                    let mut s = Scenario::empty(&ui.primary.map);
                    s.scenario_name = "just buses".to_string();
                    s.seed_buses = true;
                    s
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
        overlays: &mut Overlays,
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
                    return Some(Transition::Push(msg("Help", vec!["This simulation is empty by default.", "Try right-clicking an intersection and choosing to spawn agents (or just hover over it and press Z).", "You can also spawn agents from buildings or lanes.", "You can also start a full scenario to get realistic traffic."])));
                }
                if let Some(new_state) = spawner::AgentSpawner::new(ctx, ui) {
                    return Some(Transition::Push(new_state));
                }
                if let Some(new_state) = spawner::SpawnManyAgents::new(ctx, ui) {
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
                    return Some(Transition::Push(msg(
                        "Help",
                        vec![
                            "Do things seem a bit quiet?",
                            "The simulation starts at midnight, so you might need to wait a bit.",
                            "Try using the speed controls on the left.",
                        ],
                    )));
                }
            }
            State::OptimizeBus {
                route,
                ref mut time,
                ref mut stat,
            } => {
                self.menu.event(ctx);
                if manage_overlays(
                    &mut self.menu,
                    ctx,
                    "show bus route",
                    "hide bus route",
                    overlays,
                    match overlays {
                        Overlays::BusRoute(_) => true,
                        _ => false,
                    },
                    *time != ui.primary.sim.time(),
                ) {
                    *overlays = Overlays::BusRoute(bus_explorer::ShowBusRoute::new(
                        ui.primary.map.get_br(route),
                        ui,
                        ctx,
                    ));
                }
                if manage_overlays(
                    &mut self.menu,
                    ctx,
                    "show delays over time",
                    "hide delays over time",
                    overlays,
                    match overlays {
                        Overlays::BusDelaysOverTime(_) => true,
                        _ => false,
                    },
                    *time != ui.primary.sim.time(),
                ) {
                    *overlays = Overlays::BusDelaysOverTime(bus_delays(route, ui, ctx));
                }

                // TODO Expensive
                if *time != ui.primary.sim.time() {
                    *time = ui.primary.sim.time();
                    self.menu
                        .set_info(ctx, bus_route_panel(route, ui, *stat, &self.prebaked));
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
                    return Some(Transition::Push(msg(
                        "Help",
                        vec![
                            "First find where the bus gets stuck.",
                            "Then use edit mode to try to speed things up.",
                            "Try making dedicated bus lanes",
                            "and adjusting traffic signals.",
                        ],
                    )));
                }
            }
            State::CreateGridlock { ref mut time } => {
                self.menu.event(ctx);
                manage_acs(
                    &mut self.menu,
                    ctx,
                    ui,
                    "show agent delay",
                    "hide agent delay",
                    AgentColorScheme::Delay,
                );

                if *time != ui.primary.sim.time() {
                    *time = ui.primary.sim.time();
                    self.menu.set_info(ctx, gridlock_panel(ui, &self.prebaked));
                }

                if self.menu.action("help") {
                    return Some(Transition::Push(msg("Help", vec![
                        "You might notice a few places in the map where gridlock forms already.",
                        "You can make things worse!",
                        "How few lanes can you close for construction before everything grinds to a halt?",
                    ])));
                }
            }
            State::FasterTrips { mode, ref mut time } => {
                self.menu.event(ctx);

                if *time != ui.primary.sim.time() {
                    *time = ui.primary.sim.time();
                    self.menu
                        .set_info(ctx, faster_trips_panel(mode, ui, &self.prebaked));
                }

                if self.menu.action("help") {
                    return Some(Transition::Push(msg(
                        "Help",
                        vec!["How can you possibly speed up all trips of some mode?"],
                    )));
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }
}

fn bus_route_panel(id: BusRouteID, ui: &UI, stat: Statistic, prebaked: &Analytics) -> Text {
    let now = ui
        .primary
        .sim
        .get_analytics()
        .bus_arrivals(ui.primary.sim.time(), id);
    let baseline = prebaked.bus_arrivals(ui.primary.sim.time(), id);

    let route = ui.primary.map.get_br(id);
    let mut txt = Text::new();
    txt.add(Line(format!("{} delay between stops", stat)));
    for idx1 in 0..route.stops.len() {
        let idx2 = if idx1 == route.stops.len() - 1 {
            0
        } else {
            idx1 + 1
        };
        // TODO Also display number of arrivals...
        txt.add(Line(format!("Stop {}->{}: ", idx1 + 1, idx2 + 1)));
        if let Some(ref stats1) = now.get(&route.stops[idx2]) {
            let a = stats1.select(stat);
            txt.append(Line(a.minimal_tostring()));

            if let Some(ref stats2) = baseline.get(&route.stops[idx2]) {
                txt.append_all(cmp_duration_shorter(a, stats2.select(stat)));
            }
        } else {
            txt.append(Line("no arrivals yet"));
        }
    }
    txt
}

fn bus_delays(id: BusRouteID, ui: &UI, ctx: &mut EventCtx) -> Plot<Duration> {
    let route = ui.primary.map.get_br(id);
    let mut delays_per_stop = ui
        .primary
        .sim
        .get_analytics()
        .bus_arrivals_over_time(ui.primary.sim.time(), id);

    let mut series = Vec::new();
    for idx1 in 0..route.stops.len() {
        let idx2 = if idx1 == route.stops.len() - 1 {
            0
        } else {
            idx1 + 1
        };
        series.push(Series {
            label: format!("Stop {}->{}", idx1 + 1, idx2 + 1),
            color: rotating_color_total(idx1, route.stops.len()),
            pts: delays_per_stop
                .remove(&route.stops[idx2])
                .unwrap_or_else(Vec::new),
        });
    }
    Plot::new(
        &format!("delays for {}", route.name),
        series,
        Duration::ZERO,
        ctx,
    )
}

fn gridlock_panel(ui: &UI, prebaked: &Analytics) -> Text {
    let (now_all, now_per_mode) = ui
        .primary
        .sim
        .get_analytics()
        .all_finished_trips(ui.primary.sim.time());
    let (baseline_all, baseline_per_mode) = prebaked.all_finished_trips(ui.primary.sim.time());

    let mut txt = Text::new();
    txt.add_appended(vec![
        Line(format!(
            "{} total finished trips (",
            prettyprint_usize(now_all.count())
        )),
        cmp_count_fewer(now_all.count(), baseline_all.count()),
        Line(")"),
    ]);

    for mode in TripMode::all() {
        let a = now_per_mode[&mode].count();
        let b = baseline_per_mode[&mode].count();
        txt.add_appended(vec![
            Line(format!("  {}: {} (", mode, prettyprint_usize(a))),
            cmp_count_fewer(a, b),
            Line(")"),
        ]);
    }

    txt
}

fn faster_trips_panel(mode: TripMode, ui: &UI, prebaked: &Analytics) -> Text {
    let now = ui
        .primary
        .sim
        .get_analytics()
        .finished_trips(ui.primary.sim.time(), mode);
    let baseline = prebaked.finished_trips(ui.primary.sim.time(), mode);

    let mut txt = Text::new();
    txt.add_appended(vec![
        Line(format!(
            "{} finished {} trips (",
            prettyprint_usize(now.count()),
            mode
        )),
        cmp_count_more(now.count(), baseline.count()),
        Line(")"),
    ]);
    if now.count() == 0 || baseline.count() == 0 {
        return txt;
    }

    for stat in Statistic::all() {
        txt.add(Line(format!("{}: ", stat)));
        txt.append_all(cmp_duration_shorter(
            now.select(stat),
            baseline.select(stat),
        ));
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
    Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
        ctx,
        ui,
        GameplayMode::PlayScenario(scenario_name),
    ))))
}

// Must call menu.event first. Returns true if the caller should set the overlay to the custom
// thing.
fn manage_overlays(
    menu: &mut ModalMenu,
    ctx: &mut EventCtx,
    show: &str,
    hide: &str,
    overlay: &mut Overlays,
    active_originally: bool,
    time_changed: bool,
) -> bool {
    // Synchronize menus if needed. Player can change these separately.
    if active_originally {
        menu.maybe_change_action(show, hide, ctx);
    } else {
        menu.maybe_change_action(hide, show, ctx);
    }

    if !active_originally && menu.swap_action(show, hide, ctx) {
        true
    } else if active_originally && menu.swap_action(hide, show, ctx) {
        *overlay = Overlays::Inactive;
        false
    } else {
        active_originally && time_changed
    }
}

// Must call menu.event first.
fn manage_acs(
    menu: &mut ModalMenu,
    ctx: &mut EventCtx,
    ui: &mut UI,
    show: &str,
    hide: &str,
    acs: AgentColorScheme,
) {
    let active_originally = ui.agent_cs == acs;

    // Synchronize menus if needed. Player can change these separately.
    if active_originally {
        menu.maybe_change_action(show, hide, ctx);
    } else {
        menu.maybe_change_action(hide, show, ctx);
    }

    if !active_originally && menu.swap_action(show, hide, ctx) {
        ui.agent_cs = acs;
    } else if active_originally && menu.swap_action(hide, show, ctx) {
        ui.agent_cs = AgentColorScheme::VehicleTypes;
    }
}

// Shorter is better
fn cmp_duration_shorter(now: Duration, baseline: Duration) -> Vec<TextSpan> {
    if now.epsilon_eq(baseline) {
        vec![Line(" (same as baseline)")]
    } else if now < baseline {
        vec![
            Line(" ("),
            Line((baseline - now).minimal_tostring()).fg(Color::GREEN),
            Line(" faster)"),
        ]
    } else if now > baseline {
        vec![
            Line(" ("),
            Line((now - baseline).minimal_tostring()).fg(Color::RED),
            Line(" slower)"),
        ]
    } else {
        unreachable!()
    }
}

// Fewer is better
fn cmp_count_fewer(now: usize, baseline: usize) -> TextSpan {
    if now < baseline {
        Line(format!("{} fewer", prettyprint_usize(baseline - now))).fg(Color::GREEN)
    } else if now > baseline {
        Line(format!("{} more", prettyprint_usize(now - baseline))).fg(Color::RED)
    } else {
        Line("same as baseline")
    }
}

// More is better
fn cmp_count_more(now: usize, baseline: usize) -> TextSpan {
    if now < baseline {
        Line(format!("{} fewer", prettyprint_usize(baseline - now))).fg(Color::RED)
    } else if now > baseline {
        Line(format!("{} more", prettyprint_usize(now - baseline))).fg(Color::GREEN)
    } else {
        Line("same as baseline")
    }
}
