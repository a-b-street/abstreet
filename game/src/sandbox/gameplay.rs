use crate::challenges::{BusArrivals, FasterTrips, GridlockDelays, PrebakedResults};
use crate::game::{msg, Transition, WizardState};
use crate::render::AgentColorScheme;
use crate::sandbox::{analytics, bus_explorer, spawner, SandboxMode};
use crate::ui::UI;
use abstutil::{prettyprint_usize, Timer};
use ezgui::{hotkey, Choice, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text, Wizard};
use geom::{Duration, Statistic};
use map_model::BusRouteID;
use sim::{Scenario, TripMode};

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
    prebaked: PrebakedResults,
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
        let prebaked: PrebakedResults =
            abstutil::read_json("../data/prebaked_results.json", &mut Timer::throwaway()).unwrap();

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
                        &format!("Speed up {:?} trips", trip_mode),
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
                if manage_analytics(
                    &mut self.menu,
                    ctx,
                    "show bus route",
                    "hide bus route",
                    analytics,
                    match analytics {
                        analytics::Analytics::BusRoute(_) => true,
                        _ => false,
                    },
                    *time != ui.primary.sim.time(),
                ) {
                    *analytics = analytics::Analytics::BusRoute(bus_explorer::ShowBusRoute::new(
                        ui.primary.map.get_br(route),
                        ui,
                        ctx,
                    ));
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

fn bus_route_panel(id: BusRouteID, ui: &UI, stat: Statistic, prebaked: &PrebakedResults) -> Text {
    let now = BusArrivals::from(&ui.primary.sim).to_stats(id, ui.primary.sim.time());
    let baseline = prebaked.bus_arrivals.to_stats(id, ui.primary.sim.time());

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
            let us = stats1.stats[&stat];
            txt.append(Line(us.minimal_tostring()));

            if let Some(ref stats2) = baseline.get(&route.stops[idx2]) {
                let vs = stats2.stats[&stat];
                if us <= vs {
                    txt.append(Line(" ("));
                    txt.append(Line((vs - us).minimal_tostring()).fg(Color::GREEN));
                    txt.append(Line(" faster)"));
                } else {
                    txt.append(Line(" ("));
                    txt.append(Line((us - vs).minimal_tostring()).fg(Color::RED));
                    txt.append(Line(" slower)"));
                }
            }
        } else {
            txt.append(Line("no arrivals yet"));
        }
    }
    txt
}

fn gridlock_panel(ui: &UI, prebaked: &PrebakedResults) -> Text {
    let now = GridlockDelays::from(&ui.primary.sim);
    let baseline = &prebaked.gridlock_delays;

    let now_total = (now.lt_1m + now.lt_5m + now.stuck) as f64;
    let baseline_total = (baseline.lt_1m + baseline.lt_5m + baseline.stuck) as f64;

    let mut txt = Text::new();
    txt.add(Line("How long have agents been stuck?"));
    txt.add(Line(format!(
        "under 1 min: {} ({:.1}%, vs {:.1}%)",
        prettyprint_usize(now.lt_1m),
        (now.lt_1m as f64) / now_total * 100.0,
        (baseline.lt_1m as f64) / baseline_total * 100.0
    )));
    txt.add(Line(format!(
        "under 5 mins: {} ({:.1}%, vs {:.1}%)",
        prettyprint_usize(now.lt_5m),
        (now.lt_5m as f64) / now_total * 100.0,
        (baseline.lt_5m as f64) / baseline_total * 100.0
    )));
    txt.add(Line(format!(
        "over 5 mins: {} ({:.1}%, vs {:.1}%)",
        prettyprint_usize(now.stuck),
        (now.stuck as f64) / now_total * 100.0,
        (baseline.stuck as f64) / baseline_total * 100.0
    )));
    txt
}

fn faster_trips_panel(mode: TripMode, ui: &UI, prebaked: &PrebakedResults) -> Text {
    let now = FasterTrips::from(&ui.primary.sim)
        .to_stats(ui.primary.sim.time())
        .remove(&mode)
        .unwrap();
    let baseline = prebaked
        .faster_trips
        .to_stats(ui.primary.sim.time())
        .remove(&mode)
        .unwrap();

    let mut txt = Text::new();
    txt.add(Line(format!(
        "{} finished {:?} trips (vs {})",
        prettyprint_usize(now.count),
        mode,
        prettyprint_usize(baseline.count),
    )));
    if now.count == 0 || baseline.count == 0 {
        return txt;
    }

    // TODO Which one?
    if false {
        for (stat, dt) in &now.stats {
            txt.add(Line(format!("{}: ", stat)));
            let vs = baseline.stats[&stat];
            let color = if *dt <= vs { Color::GREEN } else { Color::RED };
            txt.append(Line(dt.minimal_tostring()).fg(color));
            txt.append(Line(format!(" (vs {})", vs.minimal_tostring())));
        }
    }
    if true {
        for (stat, dt) in &now.stats {
            txt.add(Line(format!("{}: ", stat)));
            let vs = baseline.stats[&stat];
            if *dt <= vs {
                txt.append(Line((vs - *dt).minimal_tostring()).fg(Color::GREEN));
                txt.append(Line(" faster"));
            } else {
                txt.append(Line((*dt - vs).minimal_tostring()).fg(Color::RED));
                txt.append(Line(" slower"));
            }
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
    Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
        ctx,
        ui,
        GameplayMode::PlayScenario(scenario_name),
    ))))
}

// Must call menu.event first. Returns true if the caller should set the analytics to the custom
// thing.
fn manage_analytics(
    menu: &mut ModalMenu,
    ctx: &mut EventCtx,
    show: &str,
    hide: &str,
    analytics: &mut analytics::Analytics,
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
        *analytics = analytics::Analytics::Inactive;
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
