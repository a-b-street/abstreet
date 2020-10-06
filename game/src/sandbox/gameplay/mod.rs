use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::Duration;
use map_model::{EditCmd, EditIntersection, Map, MapEdits};
use sim::{Analytics, OrigPersonID, Scenario, ScenarioGenerator, ScenarioModifier};
use widgetry::{
    lctrl, Btn, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, TextExt, Widget,
};

pub use self::freeform::spawn_agents_around;
pub use self::tutorial::{Tutorial, TutorialPointer, TutorialState};
use crate::app::App;
use crate::challenges::{Challenge, ChallengesPicker};
use crate::edit::{apply_map_edits, SaveEdits};
use crate::game::{State, Transition};
use crate::pregame::MainMenu;
use crate::sandbox::{Actions, SandboxControls, SandboxMode};

// TODO pub so challenges can grab cutscenes and SandboxMode can dispatch to actions. Weird?
pub mod commute;
pub mod fix_traffic_signals;
pub mod freeform;
pub mod play_scenario;
pub mod tutorial;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum GameplayMode {
    // TODO Maybe this should be "sandbox"
    // Map path
    Freeform(String),
    // Map path, scenario name
    PlayScenario(String, String, Vec<ScenarioModifier>),
    FixTrafficSignals,
    OptimizeCommute(OrigPersonID, Duration),

    // current
    Tutorial(TutorialPointer),
}

pub trait GameplayState: downcast_rs::Downcast {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
        actions: &mut Actions,
    ) -> Option<Transition>;
    fn draw(&self, g: &mut GfxCtx, app: &App);
    fn on_destroy(&self, _: &mut App) {}

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
    fn has_agent_meter(&self) -> bool {
        true
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
            GameplayMode::PlayScenario(ref path, _, _) => path.to_string(),
            GameplayMode::FixTrafficSignals => abstutil::path_map("downtown"),
            GameplayMode::OptimizeCommute(_, _) => abstutil::path_map("montlake"),
            GameplayMode::Tutorial(_) => abstutil::path_map("montlake"),
        }
    }

    pub fn scenario(
        &self,
        map: &Map,
        num_agents: Option<usize>,
        mut rng: XorShiftRng,
        timer: &mut Timer,
    ) -> Option<Scenario> {
        let name = match self {
            GameplayMode::Freeform(_) => {
                let mut s = Scenario::empty(map, "empty");
                s.only_seed_buses = None;
                return Some(s);
            }
            GameplayMode::PlayScenario(_, ref scenario, _) => scenario.to_string(),
            // TODO Some of these WILL have scenarios!
            GameplayMode::Tutorial(_) => {
                return None;
            }
            _ => "weekday".to_string(),
        };
        Some(if name == "random" {
            (if let Some(n) = num_agents {
                ScenarioGenerator::scaled_run(n)
            } else {
                ScenarioGenerator::small_run(map)
            })
            .generate(map, &mut rng, timer)
        } else if name == "home_to_work" {
            ScenarioGenerator::proletariat_robot(map, &mut rng, timer)
        } else {
            let path = abstutil::path_scenario(map.get_name(), &name);
            let mut scenario = match abstutil::read_object(path.clone(), timer) {
                Ok(s) => s,
                Err(err) => {
                    Map::corrupt_err(path, err);
                    std::process::exit(1);
                }
            };
            if let GameplayMode::PlayScenario(_, _, ref modifiers) = self {
                for m in modifiers {
                    scenario = m.apply(map, scenario, &mut rng);
                }
            }
            scenario
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

    pub fn can_jump_to_time(&self) -> bool {
        match self {
            GameplayMode::Freeform(_) => false,
            _ => true,
        }
    }

    pub fn allows(&self, edits: &MapEdits) -> bool {
        for cmd in &edits.commands {
            match cmd {
                EditCmd::ChangeRoad { .. } => {
                    if !self.can_edit_lanes() {
                        return false;
                    }
                }
                EditCmd::ChangeIntersection { ref new, .. } => match new {
                    // TODO Conflating construction
                    EditIntersection::StopSign(_) | EditIntersection::Closed => {
                        if !self.can_edit_stop_signs() {
                            return false;
                        }
                    }
                    _ => {}
                },
                EditCmd::ChangeRouteSchedule { .. } => {}
            }
        }
        true
    }

    pub fn initialize(&self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn GameplayState> {
        ctx.loading_screen("setup challenge", |ctx, timer| {
            if &abstutil::basename(&self.map_path()) != app.primary.map.get_name() {
                app.switch_map(ctx, self.map_path());
            }

            if let Some(scenario) = self.scenario(
                &app.primary.map,
                app.primary.current_flags.num_agents,
                app.primary.current_flags.sim_flags.make_rng(),
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
                    .tiny_step(&app.primary.map, &mut app.primary.sim_cb);

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
                            "No prebaked simulation results for \"{}\" scenario on {} map. This \
                             means trip dashboards can't compare current times to any kind of \
                             baseline.",
                            scenario.scenario_name, scenario.map_name
                        );
                        app.set_prebaked(None);
                    }
                }
            }
        });
        match self {
            GameplayMode::Freeform(_) => freeform::Freeform::new(ctx, app),
            GameplayMode::PlayScenario(_, ref scenario, ref modifiers) => {
                play_scenario::PlayScenario::new(ctx, app, scenario, modifiers.clone())
            }
            GameplayMode::FixTrafficSignals => {
                fix_traffic_signals::FixTrafficSignals::new(ctx, app)
            }
            GameplayMode::OptimizeCommute(p, goal) => {
                commute::OptimizeCommute::new(ctx, app, *p, *goal)
            }
            GameplayMode::Tutorial(current) => Tutorial::new(ctx, app, *current),
        }
    }
}

fn challenge_header(ctx: &mut EventCtx, title: &str) -> Widget {
    Widget::row(vec![
        Line(title).small_heading().draw(ctx).centered_vert(),
        Btn::svg_def("system/assets/tools/info.svg")
            .build(ctx, "instructions", None)
            .centered_vert(),
        Widget::vert_separator(ctx, 50.0),
        Btn::svg_def("system/assets/tools/edit_map.svg")
            .build(ctx, "edit map", lctrl(Key::E))
            .centered_vert(),
    ])
    .padding(5)
}

pub struct FinalScore {
    panel: Panel,
    retry: GameplayMode,
    next_mode: Option<GameplayMode>,

    chose_next: bool,
    chose_back_to_challenges: bool,
}

impl FinalScore {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        msg: String,
        mode: GameplayMode,
        next_mode: Option<GameplayMode>,
    ) -> Box<dyn State> {
        Box::new(FinalScore {
            panel: Panel::new(
                Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx.prerender, "system/assets/characters/boss.svg")
                            .scale(0.75)
                            .autocrop(),
                    )
                    .container()
                    .outline(10.0, Color::BLACK)
                    .padding(10),
                    Widget::col(vec![
                        msg.draw_text(ctx),
                        // TODO Adjust wording
                        Btn::text_bg2("Keep simulating").build_def(ctx, None),
                        Btn::text_bg2("Try again").build_def(ctx, None),
                        if next_mode.is_some() {
                            Btn::text_bg2("Next challenge").build_def(ctx, None)
                        } else {
                            Widget::nothing()
                        },
                        Btn::text_bg2("Back to challenges").build_def(ctx, None),
                    ])
                    .outline(10.0, Color::BLACK)
                    .padding(10),
                ])
                .bg(app.cs.panel_bg),
            )
            .build_custom(ctx),
            retry: mode,
            next_mode,
            chose_next: false,
            chose_back_to_challenges: false,
        })
    }
}

impl State for FinalScore {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Keep simulating" => {
                    return Transition::Pop;
                }
                "Try again" => {
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(SandboxMode::new(ctx, app, self.retry.clone())),
                    ]);
                }
                "Next challenge" => {
                    self.chose_next = true;
                    if app.primary.map.unsaved_edits() {
                        return Transition::Push(SaveEdits::new(
                            ctx,
                            app,
                            "Do you want to save your proposal first?",
                            true,
                            None,
                            Box::new(|_, _| {}),
                        ));
                    }
                }
                "Back to challenges" => {
                    self.chose_back_to_challenges = true;
                    if app.primary.map.unsaved_edits() {
                        return Transition::Push(SaveEdits::new(
                            ctx,
                            app,
                            "Do you want to save your proposal first?",
                            true,
                            None,
                            Box::new(|_, _| {}),
                        ));
                    }
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if self.chose_next || self.chose_back_to_challenges {
            ctx.loading_screen("reset map and sim", |ctx, mut timer| {
                // Always safe to do this
                apply_map_edits(ctx, app, app.primary.map.new_edits());
                app.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);

                app.primary.clear_sim();
                app.set_prebaked(None);
            });
        }

        if self.chose_next {
            return Transition::Clear(vec![
                MainMenu::new(ctx, app),
                SandboxMode::new(ctx, app, self.next_mode.clone().unwrap()),
                (Challenge::find(self.next_mode.as_ref().unwrap())
                    .0
                    .cutscene
                    .unwrap())(ctx, app, self.next_mode.as_ref().unwrap()),
            ]);
        }
        if self.chose_back_to_challenges {
            return Transition::Clear(vec![
                MainMenu::new(ctx, app),
                ChallengesPicker::new(ctx, app),
            ]);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // Happens to be a nice background color too ;)
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}
