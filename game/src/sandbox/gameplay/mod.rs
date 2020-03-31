mod create_gridlock;
mod faster_trips;
mod fix_traffic_signals;
mod freeform;
mod optimize_bus;
mod play_scenario;
pub mod spawner;
mod tutorial;

pub use self::tutorial::{Tutorial, TutorialPointer, TutorialState};
use crate::app::App;
use crate::challenges;
use crate::challenges::challenges_picker;
use crate::colors;
use crate::common::{CommonState, ContextualActions};
use crate::edit::EditMode;
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::managed::WrappedComposite;
use crate::pregame::main_menu;
use crate::sandbox::{SandboxControls, SandboxMode, ScoreCard};
use abstutil::Timer;
use ezgui::{
    lctrl, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Text, VerticalAlignment, Widget,
};
use geom::{Duration, Polygon};
use map_model::{EditCmd, EditIntersection, Map, MapEdits};
use sim::{Analytics, Scenario, TripMode};

#[derive(PartialEq, Clone)]
pub enum GameplayMode {
    // TODO Maybe this should be "sandbox"
    // Map path
    Freeform(String),
    // Map path, scenario name
    PlayScenario(String, String),
    // Map path, Route name
    OptimizeBus(String, String),
    // Map path
    CreateGridlock(String),
    // TODO Be able to filter population by more factors
    // Map path
    FasterTrips(String, TripMode),
    FixTrafficSignals,
    // TODO Kinda gross. What stage in the tutorial?
    FixTrafficSignalsTutorial(usize),

    // current
    Tutorial(TutorialPointer),
}

pub trait GameplayState: downcast_rs::Downcast {
    // True if we should exit the sandbox mode.
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
    ) -> (Option<Transition>, bool);
    fn draw(&self, g: &mut GfxCtx, app: &App);

    fn can_move_canvas(&self) -> bool {
        true
    }
    fn can_examine_objects(&self) -> bool {
        true
    }
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
    fn get_agent_meter_params(&self) -> Option<Option<ScoreCard>> {
        Some(None)
    }
    fn has_minimap(&self) -> bool {
        true
    }
}
downcast_rs::impl_downcast!(GameplayState);

impl GameplayMode {
    pub fn map_path(&self) -> String {
        match self {
            GameplayMode::Freeform(ref path) => path.to_string(),
            GameplayMode::PlayScenario(ref path, _) => path.to_string(),
            GameplayMode::OptimizeBus(ref path, _) => path.to_string(),
            GameplayMode::CreateGridlock(ref path) => path.to_string(),
            GameplayMode::FasterTrips(ref path, _) => path.to_string(),
            GameplayMode::FixTrafficSignals => abstutil::path_map("montlake"),
            GameplayMode::FixTrafficSignalsTutorial(_) => {
                abstutil::path_synthetic_map("signal_single")
            }
            GameplayMode::Tutorial(_) => abstutil::path_map("montlake"),
        }
    }

    pub fn scenario(
        &self,
        map: &Map,
        num_agents: Option<usize>,
        timer: &mut Timer,
    ) -> Option<Scenario> {
        let name = match self {
            GameplayMode::Freeform(_) => {
                return None;
            }
            GameplayMode::PlayScenario(_, ref scenario) => scenario.to_string(),
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
                EditCmd::ChangeIntersection { ref new, .. } => match new {
                    EditIntersection::StopSign(_) => {
                        if !self.can_edit_stop_signs() {
                            return false;
                        }
                    }
                    _ => {}
                },
            }
        }
        true
    }

    pub fn initialize(&self, app: &mut App, ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        ctx.loading_screen("setup challenge", |ctx, timer| {
            if &abstutil::basename(&self.map_path()) != app.primary.map.get_name() {
                app.switch_map(ctx, self.map_path());
            }

            if let Some(scenario) = self.scenario(
                &app.primary.map,
                app.primary.current_flags.num_agents,
                timer,
            ) {
                scenario.instantiate(
                    &mut app.primary.sim,
                    &app.primary.map,
                    &mut app.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                app.primary
                    .sim
                    .normal_step(&app.primary.map, Duration::seconds(0.1));

                // Maybe we've already got prebaked data for this map+scenario.
                if !app
                    .has_prebaked()
                    .map(|(m, s)| m == &scenario.map_name && s == &scenario.scenario_name)
                    .unwrap_or(false)
                {
                    // If there's no prebaked data, so be it; some functionality disappears
                    if let Ok(prebaked) = abstutil::maybe_read_binary::<Analytics>(
                        abstutil::path_prebaked_results(
                            &scenario.map_name,
                            &scenario.scenario_name,
                        ),
                        timer,
                    ) {
                        app.set_prebaked(Some((
                            scenario.map_name.clone(),
                            scenario.scenario_name.clone(),
                            prebaked,
                        )));
                    } else {
                        println!(
                            "WARNING: No prebaked results for {} on {}, some stuff might break",
                            scenario.scenario_name, scenario.map_name
                        );
                        app.set_prebaked(None);
                    }
                }
            }
        });
        match self {
            GameplayMode::Freeform(_) => freeform::Freeform::new(ctx, app, self.clone()),
            GameplayMode::PlayScenario(_, ref scenario) => {
                play_scenario::PlayScenario::new(ctx, app, scenario, self.clone())
            }
            GameplayMode::OptimizeBus(_, ref route_name) => {
                optimize_bus::OptimizeBus::new(ctx, app, route_name, self.clone())
            }
            GameplayMode::CreateGridlock(_) => {
                create_gridlock::CreateGridlock::new(ctx, self.clone())
            }
            GameplayMode::FasterTrips(_, trip_mode) => {
                faster_trips::FasterTrips::new(ctx, *trip_mode, self.clone())
            }
            GameplayMode::FixTrafficSignals | GameplayMode::FixTrafficSignalsTutorial(_) => {
                fix_traffic_signals::FixTrafficSignals::new(ctx, self.clone())
            }
            GameplayMode::Tutorial(current) => Tutorial::new(ctx, app, *current),
        }
    }
}

impl ContextualActions for GameplayMode {
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)> {
        match self {
            GameplayMode::Freeform(_) => spawner::actions(app, id),
            GameplayMode::Tutorial(_) => tutorial::actions(app, id),
            _ => Vec::new(),
        }
    }

    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        _: &mut bool,
    ) -> Transition {
        match self {
            GameplayMode::Freeform(_) => spawner::execute(ctx, app, id, action),
            GameplayMode::Tutorial(_) => tutorial::execute(ctx, app, id, action),
            _ => unreachable!(),
        }
    }
}

fn challenge_controller(
    ctx: &mut EventCtx,
    gameplay: GameplayMode,
    title: &str,
    extra_rows: Vec<Widget>,
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

    let mut rows = vec![Widget::row(vec![
        Line(title).small_heading().draw(ctx).margin(5),
        Btn::svg_def("../data/system/assets/tools/info.svg")
            .build(ctx, "instructions", None)
            .margin(5),
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
        )
        .margin(5),
        Btn::svg_def("../data/system/assets/tools/edit_map.svg")
            .build(ctx, "edit map", lctrl(Key::E))
            .margin(5),
    ])
    .centered()];
    rows.extend(extra_rows);

    WrappedComposite::new(
        Composite::new(Widget::col(rows).bg(colors::PANEL_BG))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
    )
    .cb(
        "edit map",
        Box::new(move |ctx, app| {
            Some(Transition::Push(Box::new(EditMode::new(
                ctx,
                app,
                gameplay.clone(),
            ))))
        }),
    )
    // TODO msg() is silly, it's hard to plumb the title. Also, show the challenge splash screen.
    .cb(
        "instructions",
        Box::new(move |_, _| Some(Transition::Push(msg("Challenge", description.clone())))),
    )
}

struct FinalScore {
    composite: Composite,
    mode: GameplayMode,
    next: Option<GameplayMode>,
}

impl FinalScore {
    fn new(
        ctx: &mut EventCtx,
        verdict: String,
        mode: GameplayMode,
        next: Option<GameplayMode>,
    ) -> Box<dyn State> {
        let mut txt = Text::from(Line("Final score").small_heading());
        txt.add(Line(verdict));

        let row = vec![
            if next.is_some() {
                Btn::text_fg("next challenge").build_def(ctx, None)
            } else {
                Widget::nothing()
            },
            Btn::text_fg("try again").build_def(ctx, None),
            Btn::text_fg("back to challenges").build_def(ctx, None),
        ];

        Box::new(FinalScore {
            composite: Composite::new(
                Widget::col(vec![txt.draw(ctx), Widget::row(row).centered()])
                    .bg(colors::PANEL_BG)
                    .outline(10.0, Color::WHITE)
                    .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
            .build(ctx),
            mode,
            next,
        })
    }
}

impl State for FinalScore {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "next challenge" => {
                    app.primary.clear_sim();
                    Transition::PopThenReplace(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        self.next.clone().unwrap(),
                    )))
                }
                "try again" => {
                    app.primary.clear_sim();
                    Transition::PopThenReplace(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        self.mode.clone(),
                    )))
                }
                "back to challenges" => {
                    app.primary.clear_sim();
                    Transition::Clear(vec![main_menu(ctx, app), challenges_picker(ctx, app)])
                }
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g);

        self.composite.draw(g);
        // Still want to show hotkeys
        CommonState::draw_osd(g, app, &None);
    }
}
