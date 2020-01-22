mod create_gridlock;
mod faster_trips;
mod fix_traffic_signals;
mod freeform;
mod optimize_bus;
mod play_scenario;
pub mod spawner;

use crate::challenges;
use crate::common::Overlays;
use crate::edit::EditMode;
use crate::game::{msg, Transition};
use crate::managed::{Composite, Outcome};
use crate::render::{AgentColorScheme, InnerAgentColorScheme};
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::{prettyprint_usize, Timer};
use ezgui::{
    lctrl, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, ManagedWidget,
    ModalMenu, Text, TextSpan, VerticalAlignment, Wizard,
};
use geom::{Duration, Polygon};
use map_model::{EditCmd, Map, MapEdits};
use sim::{Analytics, Scenario, TripMode};

pub struct GameplayRunner {
    pub mode: GameplayMode,
    // TODO Why not make each state own this?
    controller: Composite,
    state: Box<dyn GameplayState>,
}

#[derive(PartialEq, Clone)]
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
    // TODO Kinda gross. What stage in the tutorial?
    FixTrafficSignalsTutorial(usize),
}

pub trait GameplayState: downcast_rs::Downcast {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition>;
    fn draw(&self, g: &mut GfxCtx, ui: &UI);
}
downcast_rs::impl_downcast!(GameplayState);

impl GameplayMode {
    pub fn scenario(
        &self,
        map: &Map,
        num_agents: Option<usize>,
        timer: &mut Timer,
    ) -> Option<Scenario> {
        let name = match self {
            GameplayMode::Freeform => {
                return None;
            }
            GameplayMode::PlayScenario(ref scenario) => scenario.to_string(),
            GameplayMode::FixTrafficSignalsTutorial(stage) => {
                if *stage == 0 {
                    return Some(fix_traffic_signals::tutorial_scenario_lvl1(map));
                } else if *stage == 1 {
                    return Some(fix_traffic_signals::tutorial_scenario_lvl2(map));
                } else {
                    unreachable!()
                }
            }
            _ => "weekday".to_string(),
        };
        Some(if name == "random" {
            if let Some(n) = num_agents {
                Scenario::scaled_run(map, n)
            } else {
                Scenario::small_run(map)
            }
        } else if name == "just buses" {
            let mut s = Scenario::empty(map, "just buses");
            s.seed_buses = true;
            s
        } else {
            abstutil::read_binary(abstutil::path_scenario(map.get_name(), &name), timer)
        })
    }

    pub fn can_edit_lanes(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignals | GameplayMode::FixTrafficSignalsTutorial(_) => false,
            _ => true,
        }
    }

    pub fn can_edit_stop_signs(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignals | GameplayMode::FixTrafficSignalsTutorial(_) => false,
            _ => true,
        }
    }

    pub fn allows(&self, edits: &MapEdits) -> bool {
        for cmd in &edits.commands {
            match cmd {
                EditCmd::ChangeLaneType { .. } | EditCmd::ReverseLane { .. } => {
                    if !self.can_edit_lanes() {
                        return false;
                    }
                }
                EditCmd::ChangeStopSign(_) => {
                    if !self.can_edit_stop_signs() {
                        return false;
                    }
                }
                EditCmd::ChangeTrafficSignal(_)
                | EditCmd::CloseIntersection { .. }
                | EditCmd::UncloseIntersection(_, _) => {}
            }
        }
        true
    }

    pub fn has_minimap(&self) -> bool {
        match self {
            GameplayMode::FixTrafficSignalsTutorial(_) => false,
            _ => true,
        }
    }
}

impl GameplayRunner {
    pub fn initialize(mode: GameplayMode, ui: &mut UI, ctx: &mut EventCtx) -> GameplayRunner {
        let (controller, state) = match mode.clone() {
            GameplayMode::Freeform => freeform::Freeform::new(ctx, ui),
            GameplayMode::PlayScenario(scenario) => {
                play_scenario::PlayScenario::new(&scenario, ctx, ui)
            }
            GameplayMode::OptimizeBus(route_name) => {
                optimize_bus::OptimizeBus::new(route_name, ctx, ui)
            }
            GameplayMode::CreateGridlock => create_gridlock::CreateGridlock::new(ctx),
            GameplayMode::FasterTrips(trip_mode) => faster_trips::FasterTrips::new(trip_mode, ctx),
            GameplayMode::FixTrafficSignals | GameplayMode::FixTrafficSignalsTutorial(_) => {
                fix_traffic_signals::FixTrafficSignals::new(ctx, mode.clone())
            }
        };
        ctx.loading_screen("instantiate scenario", |_, timer| {
            if let Some(scenario) =
                mode.scenario(&ui.primary.map, ui.primary.current_flags.num_agents, timer)
            {
                scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));

                match mode {
                    GameplayMode::Freeform | GameplayMode::PlayScenario(_) => {}
                    // If there's no prebaked data, so be it; some functionality disappears
                    _ => {
                        if let Ok(prebaked) = abstutil::maybe_read_binary::<Analytics>(
                            abstutil::path_prebaked_results(
                                &scenario.map_name,
                                &scenario.scenario_name,
                            ),
                            timer,
                        ) {
                            ui.set_prebaked(Some(prebaked));
                        } else {
                            println!(
                                "WARNING: No prebaked results for {} on {}, some stuff might break",
                                scenario.scenario_name, scenario.map_name
                            );
                        }
                    }
                }
            }
        });
        GameplayRunner {
            mode,
            controller,
            state,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        match self.controller.event(ctx, ui) {
            Some(Outcome::Transition(t)) => {
                return Some(t);
            }
            Some(Outcome::Clicked(_)) => unreachable!(),
            None => {}
        }
        self.state.event(ctx, ui)
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.controller.draw(g);
        self.state.draw(g, ui);
    }
}

fn change_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let scenario_name = wiz
        .wrap(ctx)
        .choose_string("Instantiate which scenario?", || {
            let mut list =
                abstutil::list_all_objects(abstutil::path_all_scenarios(ui.primary.map.get_name()));
            list.push("random".to_string());
            list.push("just buses".to_string());
            list.push("empty".to_string());
            list
        })?;
    ui.primary.clear_sim();
    Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
        ctx,
        ui,
        if scenario_name == "empty" {
            GameplayMode::Freeform
        } else {
            GameplayMode::PlayScenario(scenario_name)
        },
    ))))
}

// Must call menu.event first. Returns true if the caller should set the overlay to the custom
// thing.
fn manage_overlays(
    menu: &mut ModalMenu,
    ctx: &mut EventCtx,
    ui: &mut UI,
    show: &str,
    hide: &str,
    active_originally: bool,
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
        ui.overlay = Overlays::Inactive;
        false
    } else {
        active_originally
    }
}

// Must call menu.event first.
fn manage_acs(
    menu: &mut ModalMenu,
    ctx: &mut EventCtx,
    ui: &mut UI,
    show: &str,
    hide: &str,
    acs: InnerAgentColorScheme,
) {
    let active_originally = ui.agent_cs.acs == acs;

    // Synchronize menus if needed. Player can change these separately.
    if active_originally {
        menu.maybe_change_action(show, hide, ctx);
    } else {
        menu.maybe_change_action(hide, show, ctx);
    }

    if !active_originally && menu.swap_action(show, hide, ctx) {
        ui.agent_cs = AgentColorScheme::new(acs, &ui.cs);
    } else if active_originally && menu.swap_action(hide, show, ctx) {
        ui.agent_cs = AgentColorScheme::default(&ui.cs);
    }
}

// Shorter is better
pub fn cmp_duration_shorter(now: Duration, baseline: Duration) -> Vec<TextSpan> {
    if now.epsilon_eq(baseline) {
        vec![Line(" (same as baseline)")]
    } else if now < baseline {
        vec![
            Line(" ("),
            Line((baseline - now).to_string()).fg(Color::GREEN),
            Line(" faster)"),
        ]
    } else if now > baseline {
        vec![
            Line(" ("),
            Line((now - baseline).to_string()).fg(Color::RED),
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

pub fn challenge_controller(ctx: &mut EventCtx, gameplay: GameplayMode, title: &str) -> Composite {
    // Scrape the description
    let mut description = Vec::new();
    'OUTER: for (_, stages) in challenges::all_challenges() {
        for challenge in stages {
            if challenge.gameplay == gameplay {
                description = challenge.description.clone();
                break 'OUTER;
            }
        }
    }

    Composite::new(
        ezgui::Composite::new(
            ManagedWidget::row(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line(title).size(26))).margin(5),
                Composite::svg_button(ctx, "assets/tools/info.svg", "info", None).margin(5),
                ManagedWidget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
                )
                .margin(5),
                Composite::svg_button(ctx, "assets/tools/edit_map.svg", "edit map", lctrl(Key::E))
                    .margin(5),
            ])
            .centered()
            .bg(Color::grey(0.4)),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx),
    )
    .cb(
        "edit map",
        Box::new(move |ctx, ui| {
            Some(Transition::Replace(Box::new(EditMode::new(
                ctx,
                ui,
                gameplay.clone(),
            ))))
        }),
    )
    // TODO msg() is silly, it's hard to plumb the title. Also, show the challenge splash screen.
    .cb(
        "info",
        Box::new(move |_, _| Some(Transition::Push(msg("Challenge", description.clone())))),
    )
}
