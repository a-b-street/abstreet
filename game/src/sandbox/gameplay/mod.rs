mod create_gridlock;
mod faster_trips;
mod fix_traffic_signals;
mod freeform;
mod optimize_bus;
mod play_scenario;
pub mod spawner;

use crate::game::Transition;
use crate::render::AgentColorScheme;
use crate::sandbox::overlays::Overlays;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::{prettyprint_usize, Timer};
use ezgui::{Color, EventCtx, GfxCtx, Line, ModalMenu, TextSpan, Wizard};
use geom::Duration;
use sim::{Scenario, TripMode};

pub struct GameplayRunner {
    pub mode: GameplayMode,
    pub menu: ModalMenu,
    state: Box<dyn GameplayState>,
}

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
    FixTrafficSignals,
}

pub trait GameplayState: downcast_rs::Downcast {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        overlays: &mut Overlays,
        menu: &mut ModalMenu,
    ) -> Option<Transition>;
    fn draw(&self, _: &mut GfxCtx, _: &UI) {}
}
downcast_rs::impl_downcast!(GameplayState);

impl GameplayMode {
    pub fn scenario(&self, ui: &UI, timer: &mut Timer) -> Option<Scenario> {
        let name = match self {
            GameplayMode::Freeform => {
                return None;
            }
            GameplayMode::PlayScenario(ref scenario) => scenario,
            _ => "weekday_typical_traffic_from_psrc",
        };
        let num_agents = ui.primary.current_flags.num_agents;
        let builtin = if let Some(n) = num_agents {
            format!("random scenario with {} agents", n)
        } else {
            "random scenario with some agents".to_string()
        };
        Some(if name == builtin {
            if let Some(n) = num_agents {
                Scenario::scaled_run(&ui.primary.map, n)
            } else {
                Scenario::small_run(&ui.primary.map)
            }
        } else if name == "just buses" {
            let mut s = Scenario::empty(&ui.primary.map);
            s.scenario_name = "just buses".to_string();
            s.seed_buses = true;
            s
        } else {
            abstutil::read_binary(
                &abstutil::path1_bin(&ui.primary.map.get_name(), abstutil::SCENARIOS, &name),
                timer,
            )
            .unwrap()
        })
    }

    pub fn can_edit_lanes(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignals => false,
            _ => true,
        }
    }

    pub fn can_edit_stop_signs(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignals => false,
            _ => true,
        }
    }
}

impl GameplayRunner {
    pub fn initialize(mode: GameplayMode, ui: &mut UI, ctx: &mut EventCtx) -> GameplayRunner {
        let (menu, state) = match mode.clone() {
            GameplayMode::Freeform => freeform::Freeform::new(ctx),
            GameplayMode::PlayScenario(scenario) => {
                play_scenario::PlayScenario::new(&scenario, ctx)
            }
            GameplayMode::OptimizeBus(route_name) => {
                optimize_bus::OptimizeBus::new(route_name, ctx, ui)
            }
            GameplayMode::CreateGridlock => create_gridlock::CreateGridlock::new(ctx),
            GameplayMode::FasterTrips(trip_mode) => faster_trips::FasterTrips::new(trip_mode, ctx),
            GameplayMode::FixTrafficSignals => fix_traffic_signals::FixTrafficSignals::new(ctx),
        };
        ctx.loading_screen("instantiate scenario", |_, timer| {
            if let Some(scenario) = mode.scenario(ui, timer) {
                scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
            }
        });
        GameplayRunner {
            mode,
            menu: menu.disable_standalone_layout(),
            state,
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        overlays: &mut Overlays,
    ) -> Option<Transition> {
        self.state.event(ctx, ui, overlays, &mut self.menu)
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.menu.draw(g);
        self.state.draw(g, ui);
    }
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
    ui.primary.clear_sim();
    Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
        ctx,
        ui,
        GameplayMode::PlayScenario(scenario_name),
    ))))
}

fn load_map(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    if let Some(name) = wiz.wrap(ctx).choose_string("Load which map?", || {
        let current_map = ui.primary.map.get_name();
        abstutil::list_all_objects("maps", "")
            .into_iter()
            .filter(|n| n != current_map)
            .collect()
    }) {
        ui.switch_map(ctx, &name);
        Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
            ctx,
            ui,
            // TODO If we were playing a scenario, load that one...
            GameplayMode::Freeform,
        ))))
    } else if wiz.aborted() {
        Some(Transition::Pop)
    } else {
        None
    }
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
pub fn cmp_duration_shorter(now: Duration, baseline: Duration) -> Vec<TextSpan> {
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
pub fn cmp_count_fewer(now: usize, baseline: usize) -> TextSpan {
    if now < baseline {
        Line(format!("{} fewer", prettyprint_usize(baseline - now))).fg(Color::GREEN)
    } else if now > baseline {
        Line(format!("{} more", prettyprint_usize(now - baseline))).fg(Color::RED)
    } else {
        Line("same as baseline")
    }
}

// More is better
pub fn cmp_count_more(now: usize, baseline: usize) -> TextSpan {
    if now < baseline {
        Line(format!("{} fewer", prettyprint_usize(baseline - now))).fg(Color::RED)
    } else if now > baseline {
        Line(format!("{} more", prettyprint_usize(now - baseline))).fg(Color::GREEN)
    } else {
        Line("same as baseline")
    }
}
