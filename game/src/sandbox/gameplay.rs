use crate::game::Transition;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Line, ModalMenu, Text};
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
    OptimizeBus { route: BusRouteID, time: Duration },
}

impl GameplayState {
    pub fn initialize(mode: GameplayMode, ui: &mut UI, ctx: &mut EventCtx) -> GameplayState {
        // TODO Instantiate scenario, often weekday_typical_traffic_from_psrc
        let (state, maybe_scenario) = match mode.clone() {
            GameplayMode::Freeform => (
                GameplayState {
                    mode,
                    // TODO play a scenario instead
                    menu: ModalMenu::new("Freeform mode", vec![], ctx).disable_standalone_layout(),
                    state: State::Freeform,
                },
                None,
            ),
            GameplayMode::PlayScenario(scenario) => (
                GameplayState {
                    mode,
                    // TODO play different scenario instead
                    menu: ModalMenu::new(&format!("Playing {}", scenario), vec![], ctx)
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
                        // TODO open route
                        menu: ModalMenu::new(&format!("Optimize {}", route_name), vec![], ctx)
                            .disable_standalone_layout(),
                        state: State::OptimizeBus {
                            route: route.id,
                            time: Duration::ZERO,
                        },
                    },
                    Some("weekday_typical_traffic_from_psrc".to_string()),
                )
            }
        };
        if let Some(scenario) = maybe_scenario {
            ctx.loading_screen("instantiate scenario", |_, timer| {
                let scenario: Scenario = abstutil::read_binary(
                    &abstutil::path1_bin(ui.primary.map.get_name(), abstutil::SCENARIOS, &scenario),
                    timer,
                )
                .unwrap();
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

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> Option<Transition> {
        match self.state {
            State::Freeform => {
                // TODO agent spawner
            }
            State::PlayScenario => {}
            State::OptimizeBus {
                route,
                ref mut time,
            } => {
                if *time != ui.primary.sim.time() {
                    *time = ui.primary.sim.time();
                    self.menu.set_info(ctx, bus_route_panel(route, ui));
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
