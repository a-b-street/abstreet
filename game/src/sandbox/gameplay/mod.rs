mod create_gridlock;
mod faster_trips;
mod fix_traffic_signals;
mod freeform;
mod optimize_bus;
mod play_scenario;
pub mod spawner;
mod tutorial;

pub use self::tutorial::{Tutorial, TutorialState};
use crate::challenges;
use crate::challenges::challenges_picker;
use crate::colors;
use crate::common::{CommonState, Overlays};
use crate::edit::EditMode;
use crate::game::{msg, State, Transition};
use crate::managed::WrappedComposite;
use crate::pregame::main_menu;
use crate::render::{AgentColorScheme, InnerAgentColorScheme};
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{
    lctrl, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, ModalMenu, Outcome, Text, VerticalAlignment,
};
use geom::{Duration, Polygon};
use map_model::{EditCmd, Map, MapEdits};
use sim::{Analytics, Scenario, TripMode};

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

    // current
    Tutorial(usize),
}

pub trait GameplayState: downcast_rs::Downcast {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition>;
    fn draw(&self, g: &mut GfxCtx, ui: &UI);

    fn has_common(&self) -> bool {
        true
    }
    fn has_tool_panel(&self) -> bool {
        true
    }
    fn has_time_panel(&self) -> bool {
        true
    }
    fn has_speed(&self) -> bool {
        true
    }
    fn has_agent_meter(&self) -> bool {
        true
    }
    fn has_minimap(&self) -> bool {
        true
    }
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
            // TODO Some of these WILL have scenarios!
            GameplayMode::Tutorial(_) => {
                return None;
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
            s.only_seed_buses = None;
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

    pub fn initialize(&self, ui: &mut UI, ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        ctx.loading_screen("instantiate scenario", |_, timer| {
            if let Some(scenario) =
                self.scenario(&ui.primary.map, ui.primary.current_flags.num_agents, timer)
            {
                scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));

                // If there's no prebaked data, so be it; some functionality disappears
                if let Ok(prebaked) = abstutil::maybe_read_binary::<Analytics>(
                    abstutil::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
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
        });
        match self {
            GameplayMode::Freeform => freeform::Freeform::new(ctx, ui),
            GameplayMode::PlayScenario(ref scenario) => {
                play_scenario::PlayScenario::new(scenario, ctx, ui)
            }
            GameplayMode::OptimizeBus(ref route_name) => {
                optimize_bus::OptimizeBus::new(route_name, ctx, ui)
            }
            GameplayMode::CreateGridlock => create_gridlock::CreateGridlock::new(ctx),
            GameplayMode::FasterTrips(trip_mode) => faster_trips::FasterTrips::new(*trip_mode, ctx),
            GameplayMode::FixTrafficSignals | GameplayMode::FixTrafficSignalsTutorial(_) => {
                fix_traffic_signals::FixTrafficSignals::new(ctx, ui, self.clone())
            }
            GameplayMode::Tutorial(current) => Tutorial::new(ctx, ui, *current),
        }
    }
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

fn challenge_controller(
    ctx: &mut EventCtx,
    gameplay: GameplayMode,
    title: &str,
    extra_rows: Vec<ManagedWidget>,
) -> WrappedComposite {
    // Scrape the description
    let mut description = Vec::new();
    'OUTER: for (_, stages) in challenges::all_challenges(true) {
        for challenge in stages {
            if challenge.gameplay == gameplay {
                description = challenge.description.clone();
                break 'OUTER;
            }
        }
    }

    let mut rows = vec![ManagedWidget::row(vec![
        ManagedWidget::draw_text(ctx, Text::from(Line(title).size(26))).margin(5),
        WrappedComposite::svg_button(ctx, "assets/tools/info.svg", "info", None).margin(5),
        ManagedWidget::draw_batch(
            ctx,
            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
        )
        .margin(5),
        WrappedComposite::svg_button(ctx, "assets/tools/edit_map.svg", "edit map", lctrl(Key::E))
            .margin(5),
    ])
    .centered()];
    rows.extend(extra_rows);

    WrappedComposite::new(
        Composite::new(ManagedWidget::col(rows).bg(colors::PANEL_BG))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
    )
    .cb(
        "edit map",
        Box::new(move |ctx, ui| {
            Some(Transition::Push(Box::new(EditMode::new(
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

struct FinalScore {
    composite: Composite,
    mode: GameplayMode,
}

impl FinalScore {
    fn new(ctx: &mut EventCtx, verdict: String, mode: GameplayMode) -> Box<dyn State> {
        let mut txt = Text::prompt("Final score");
        txt.add(Line(verdict));
        Box::new(FinalScore {
            composite: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::draw_text(ctx, txt),
                    ManagedWidget::row(vec![
                        WrappedComposite::text_button(ctx, "try again", None),
                        WrappedComposite::text_button(ctx, "back to challenges", None),
                    ])
                    .centered(),
                ])
                .bg(colors::PANEL_BG)
                .outline(10.0, Color::WHITE)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
            .build(ctx),
            mode,
        })
    }
}

impl State for FinalScore {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "try again" => {
                    ui.primary.clear_sim();
                    Transition::PopThenReplace(Box::new(SandboxMode::new(
                        ctx,
                        ui,
                        self.mode.clone(),
                    )))
                }
                "back to challenges" => {
                    ui.primary.clear_sim();
                    Transition::Clear(vec![main_menu(ctx, ui), challenges_picker(ctx, ui)])
                }
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // Make it clear the map can't be interacted with right now.
        g.fork_screenspace();
        // TODO - OSD height
        g.draw_polygon(
            Color::BLACK.alpha(0.5),
            &Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
        );
        g.unfork();

        self.composite.draw(g);
        // Still want to show hotkeys
        CommonState::draw_osd(g, ui, &None);
    }
}
