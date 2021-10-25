use anyhow::Result;
use maplit::btreeset;

use geom::{Circle, Distance, Time};
use map_gui::colors::ColorSchemeChoice;
use map_gui::load::{FileLoader, FutureLoader, MapLoader};
use map_gui::options::OptionsPanel;
use map_gui::render::{unzoomed_agent_radius, UnzoomedAgents};
use map_gui::tools::{ChooseSomething, Minimap, TurnExplorer, URLManager};
use map_gui::{AppLike, ID};
use sim::{Analytics, Scenario};
use widgetry::{lctrl, Choice, EventCtx, GfxCtx, Key, Outcome, Panel, State, UpdateType};

pub use self::gameplay::{spawn_agents_around, GameplayMode, TutorialPointer, TutorialState};
pub use self::minimap::MinimapController;
use self::misc_tools::{RoutePreview, TrafficRecorder};
pub use self::speed::{SpeedSetting, TimePanel};
pub use self::time_warp::TimeWarpScreen;
use crate::app::{App, Transition};
use crate::common::{tool_panel, CommonState};
use crate::debug::DebugMode;
use crate::edit::{
    can_edit_lane, EditMode, RoadEditor, SaveEdits, StopSignEditor, TrafficSignalEditor,
};
use crate::info::ContextualActions;
use crate::layer::favorites::{Favorites, ShowFavorites};
use crate::layer::PickLayer;
use crate::pregame::MainMenu;

pub mod dashboards;
pub mod gameplay;
mod minimap;
mod misc_tools;
mod speed;
mod time_warp;

pub struct SandboxMode {
    gameplay: Box<dyn gameplay::GameplayState>,
    pub gameplay_mode: GameplayMode,

    pub controls: SandboxControls,

    recalc_unzoomed_agent: Option<Time>,
    last_cs: ColorSchemeChoice,
}

pub struct SandboxControls {
    pub common: Option<CommonState>,
    route_preview: Option<RoutePreview>,
    tool_panel: Option<Panel>,
    pub time_panel: Option<TimePanel>,
    minimap: Option<Minimap<App, MinimapController>>,
}

impl SandboxMode {
    /// If you don't need to chain any transitions after the SandboxMode that rely on its resources
    /// being loaded, use this. Otherwise, see `async_new`.
    pub fn simple_new(app: &mut App, mode: GameplayMode) -> Box<dyn State<App>> {
        SandboxMode::async_new(app, mode, Box::new(|_, _| Vec::new()))
    }

    /// This does not immediately initialize anything (like loading the correct map, instantiating
    /// the scenario, etc). That means if you're chaining this call with other transitions, you
    /// probably need to defer running them using `finalize`.
    pub fn async_new(
        app: &mut App,
        mode: GameplayMode,
        finalize: Box<dyn FnOnce(&mut EventCtx, &mut App) -> Vec<Transition>>,
    ) -> Box<dyn State<App>> {
        app.primary.clear_sim();
        Box::new(SandboxLoader {
            stage: Some(LoadStage::LoadingMap),
            mode,
            finalize: Some(finalize),
        })
    }

    /// Assumes that the map and simulation have already been set up, and starts by loading
    /// prebaked data.
    pub fn start_from_savestate(app: &App) -> Box<dyn State<App>> {
        let scenario_name = app.primary.sim.get_run_name().to_string();
        Box::new(SandboxLoader {
            stage: Some(LoadStage::LoadingPrebaked(scenario_name.clone())),
            mode: GameplayMode::PlayScenario(
                app.primary.map.get_name().clone(),
                scenario_name,
                Vec::new(),
            ),
            finalize: Some(Box::new(|_, _| Vec::new())),
        })
    }

    // Just for Warping
    pub fn contextual_actions(&self) -> Actions {
        Actions {
            is_paused: self
                .controls
                .time_panel
                .as_ref()
                .map(|s| s.is_paused())
                .unwrap_or(true),
            can_interact: self.gameplay.can_examine_objects(),
            gameplay: self.gameplay_mode.clone(),
        }
    }
}

impl State<App> for SandboxMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if app.opts.toggle_day_night_colors {
            if is_daytime(app) {
                app.change_color_scheme(ctx, ColorSchemeChoice::DayMode)
            } else {
                app.change_color_scheme(ctx, ColorSchemeChoice::NightMode)
            };
        }

        if app.opts.color_scheme != self.last_cs {
            self.last_cs = app.opts.color_scheme;
            self.controls.recreate_panels(ctx, app);
            self.gameplay.recreate_panels(ctx, app);
        }

        // Do this before gameplay
        if self.gameplay.can_move_canvas() && ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.primary.map.get_gps_bounds());
        }

        let mut actions = self.contextual_actions();
        if let Some(t) = self
            .gameplay
            .event(ctx, app, &mut self.controls, &mut actions)
        {
            return t;
        }

        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        // Order here is pretty arbitrary
        if app.opts.dev && ctx.input.pressed(lctrl(Key::D)) {
            return Transition::Push(DebugMode::new_state(ctx, app));
        }

        if let Some(ref mut m) = self.controls.minimap {
            if let Some(t) = m.event(ctx, app) {
                return t;
            }
            if let Some(t) = PickLayer::update(ctx, app) {
                return t;
            }
        }

        if let Some(ref mut tp) = self.controls.time_panel {
            if let Some(t) = tp.event(ctx, app, Some(&self.gameplay_mode)) {
                return t;
            }
        }

        // We need to recalculate unzoomed agent mouseover when the mouse is still and time passes
        // (since something could move beneath the cursor), or when the mouse moves.
        if app.primary.current_selection.is_none()
            && ctx.canvas.is_unzoomed()
            && (ctx.redo_mouseover()
                || self
                    .recalc_unzoomed_agent
                    .map(|t| t != app.primary.sim.time())
                    .unwrap_or(true))
        {
            mouseover_unzoomed_agent_circle(ctx, app);
        }

        if let Some(ref mut r) = self.controls.route_preview {
            if let Some(t) = r.event(ctx, app) {
                return t;
            }
        }

        // Fragile ordering. Let this work before tool_panel, so Key::Escape from the info panel
        // beats the one to quit. And let speed update the sim before we update the info panel.
        if let Some(ref mut c) = self.controls.common {
            if let Some(t) = c.event(ctx, app, &mut actions) {
                return t;
            }
        }

        if let Some(ref mut tp) = self.controls.tool_panel {
            if let Outcome::Clicked(x) = tp.event(ctx) {
                match x.as_ref() {
                    "back" => {
                        return maybe_exit_sandbox(ctx);
                    }
                    "settings" => {
                        return Transition::Push(OptionsPanel::new_state(ctx, app));
                    }
                    _ => unreachable!(),
                }
            }
        }

        if self
            .controls
            .time_panel
            .as_ref()
            .map(|s| s.is_paused())
            .unwrap_or(true)
        {
            Transition::Keep
        } else {
            ctx.request_update(UpdateType::Game);
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if let Some(ref l) = app.primary.layer {
            l.draw(g, app);
        }

        if !app.opts.minimal_controls {
            if let Some(ref c) = self.controls.common {
                c.draw(g, app);
            } else {
                CommonState::draw_osd(g, app);
            }
            if let Some(ref tp) = self.controls.tool_panel {
                tp.draw(g);
            }
        }
        if let Some(ref tp) = self.controls.time_panel {
            tp.draw(g);
        }
        if let Some(ref m) = self.controls.minimap {
            m.draw(g, app);
        }
        if let Some(ref r) = self.controls.route_preview {
            r.draw(g);
        }

        if !app.opts.minimal_controls {
            self.gameplay.draw(g, app);
        }
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        app.primary.layer = None;
        app.primary.agents.borrow_mut().unzoomed_agents = UnzoomedAgents::new();
        self.gameplay.on_destroy(app);
    }
}

pub fn maybe_exit_sandbox(ctx: &mut EventCtx) -> Transition {
    Transition::Push(ChooseSomething::new_state(
        ctx,
        "Are you ready to leave this mode?",
        vec![
            Choice::string("keep playing"),
            Choice::string("quit to main screen").key(Key::Q),
        ],
        Box::new(|resp, ctx, app| {
            if resp == "keep playing" {
                return Transition::Pop;
            }

            if app.primary.map.unsaved_edits() {
                return Transition::Multi(vec![
                    Transition::Push(Box::new(BackToMainMenu)),
                    Transition::Push(SaveEdits::new_state(
                        ctx,
                        app,
                        "Do you want to save your proposal first?",
                        true,
                        None,
                        Box::new(|_, _| {}),
                    )),
                ]);
            }
            Transition::Replace(Box::new(BackToMainMenu))
        }),
    ))
}

struct BackToMainMenu;

impl State<App> for BackToMainMenu {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.change_color_scheme(ctx, ColorSchemeChoice::Pregame);
        app.clear_everything(ctx);
        Transition::Clear(vec![MainMenu::new_state(ctx)])
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

// pub for Warping
pub struct Actions {
    is_paused: bool,
    can_interact: bool,
    gameplay: GameplayMode,
}
impl ContextualActions for Actions {
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)> {
        let mut actions = Vec::new();
        if self.can_interact {
            match id {
                ID::Intersection(i) => {
                    if app.primary.map.get_i(i).is_traffic_signal() {
                        actions.push((Key::E, "edit traffic signal".to_string()));
                    }
                    if app.primary.map.get_i(i).is_stop_sign()
                        && self.gameplay.can_edit_stop_signs()
                    {
                        actions.push((Key::E, "edit stop sign".to_string()));
                    }
                    if app.opts.dev && app.primary.sim.num_recorded_trips().is_none() {
                        actions.push((Key::R, "record traffic here".to_string()));
                    }
                }
                ID::Lane(l) => {
                    if !app.primary.map.get_turns_from_lane(l).is_empty() {
                        actions.push((Key::Z, "explore turns from this lane".to_string()));
                    }
                    if self.gameplay.can_edit_roads() && can_edit_lane(app, l) {
                        actions.push((Key::E, "edit lane".to_string()));
                    }
                }
                ID::Building(b) => {
                    if Favorites::contains(app, b) {
                        actions.push((Key::F, "remove this building from favorites".to_string()));
                    } else {
                        actions.push((Key::F, "add this building to favorites".to_string()));
                    }
                }
                _ => {}
            }
        }
        actions.extend(match self.gameplay {
            GameplayMode::Freeform(_) => gameplay::freeform::actions(app, id),
            GameplayMode::Tutorial(_) => gameplay::tutorial::actions(app, id),
            _ => Vec::new(),
        });
        actions
    }
    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        close_panel: &mut bool,
    ) -> Transition {
        match (id, action.as_ref()) {
            (ID::Intersection(i), "edit traffic signal") => Transition::Multi(vec![
                Transition::Push(EditMode::new_state(ctx, app, self.gameplay.clone())),
                Transition::Push(TrafficSignalEditor::new_state(
                    ctx,
                    app,
                    btreeset! {i},
                    self.gameplay.clone(),
                )),
            ]),
            (ID::Intersection(i), "edit stop sign") => Transition::Multi(vec![
                Transition::Push(EditMode::new_state(ctx, app, self.gameplay.clone())),
                Transition::Push(StopSignEditor::new_state(
                    ctx,
                    app,
                    i,
                    self.gameplay.clone(),
                )),
            ]),
            (ID::Intersection(i), "record traffic here") => {
                Transition::Push(TrafficRecorder::new_state(ctx, btreeset! {i}))
            }
            (ID::Lane(l), "explore turns from this lane") => {
                Transition::Push(TurnExplorer::new_state(ctx, app, l))
            }
            (ID::Lane(l), "edit lane") => Transition::Multi(vec![
                Transition::Push(EditMode::new_state(ctx, app, self.gameplay.clone())),
                Transition::Push(RoadEditor::new_state(ctx, app, l)),
            ]),
            (ID::Building(b), "add this building to favorites") => {
                Favorites::add(app, b);
                app.primary.layer = Some(Box::new(ShowFavorites::new(ctx, app)));
                Transition::Keep
            }
            (ID::Building(b), "remove this building from favorites") => {
                Favorites::remove(app, b);
                app.primary.layer = Some(Box::new(ShowFavorites::new(ctx, app)));
                Transition::Keep
            }
            (_, "follow (run the simulation)") => {
                *close_panel = false;
                Transition::ModifyState(Box::new(|state, ctx, app| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let time_panel = mode.controls.time_panel.as_mut().unwrap();
                    assert!(time_panel.is_paused());
                    time_panel.resume(ctx, app, SpeedSetting::Realtime);
                }))
            }
            (_, "unfollow (pause the simulation)") => {
                *close_panel = false;
                Transition::ModifyState(Box::new(|state, ctx, app| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let time_panel = mode.controls.time_panel.as_mut().unwrap();
                    assert!(!time_panel.is_paused());
                    time_panel.pause(ctx, app);
                }))
            }
            (id, action) => match self.gameplay {
                GameplayMode::Freeform(_) => gameplay::freeform::execute(ctx, app, id, action),
                GameplayMode::Tutorial(_) => gameplay::tutorial::execute(ctx, app, id, action),
                _ => unreachable!(),
            },
        }
    }
    fn is_paused(&self) -> bool {
        self.is_paused
    }
    fn gameplay_mode(&self) -> GameplayMode {
        self.gameplay.clone()
    }
}

// TODO Setting SandboxMode up is quite convoluted, all in order to support asynchronously loading
// files on the web. Each LoadStage is followed in order, with some optional short-circuiting.
//
// Ideally there'd be a much simpler way to express this using Rust's async, to let the compiler
// express this state machine for us.

#[allow(clippy::large_enum_variant)]
enum LoadStage {
    LoadingMap,
    LoadingScenario,
    GotScenario(Scenario),
    // Scenario name
    LoadingPrebaked(String),
    // Scenario name, maybe prebaked data
    GotPrebaked(String, Result<Analytics>),
    Finalizing,
}

struct SandboxLoader {
    // Always exists, just a way to avoid clones
    stage: Option<LoadStage>,
    mode: GameplayMode,
    finalize: Option<Box<dyn FnOnce(&mut EventCtx, &mut App) -> Vec<Transition>>>,
}

impl State<App> for SandboxLoader {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        loop {
            match self.stage.take().unwrap() {
                LoadStage::LoadingMap => {
                    return Transition::Push(MapLoader::new_state(
                        ctx,
                        app,
                        self.mode.map_name(),
                        Box::new(|_, _| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ModifyState(Box::new(|state, _, _| {
                                    let loader = state.downcast_mut::<SandboxLoader>().unwrap();
                                    loader.stage = Some(LoadStage::LoadingScenario);
                                })),
                            ])
                        }),
                    ));
                }
                LoadStage::LoadingScenario => {
                    // TODO Can we cache the dynamically generated scenarios, like home_to_work, and
                    // avoid regenerating with this call?
                    match ctx.loading_screen("load scenario", |_, mut timer| {
                        self.mode.scenario(
                            app,
                            app.primary.current_flags.sim_flags.make_rng(),
                            &mut timer,
                        )
                    }) {
                        gameplay::LoadScenario::Nothing => {
                            app.set_prebaked(None);
                            self.stage = Some(LoadStage::Finalizing);
                            continue;
                        }
                        gameplay::LoadScenario::Scenario(scenario) => {
                            // TODO Consider using the cached app.primary.scenario, if possible.
                            self.stage = Some(LoadStage::GotScenario(scenario));
                            continue;
                        }
                        gameplay::LoadScenario::Future(future) => {
                            let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
                            let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
                            return Transition::Push(FutureLoader::<App, Scenario>::new_state(
                                ctx,
                                Box::pin(future),
                                outer_progress_rx,
                                inner_progress_rx,
                                "Loading Scenario",
                                Box::new(|_, _, scenario| {
                                    // TODO show error/retry alert?
                                    let scenario =
                                        scenario.expect("failed to load scenario from future");
                                    Transition::Multi(vec![
                                        Transition::Pop,
                                        Transition::ModifyState(Box::new(|state, _, _| {
                                            let loader =
                                                state.downcast_mut::<SandboxLoader>().unwrap();
                                            loader.stage = Some(LoadStage::GotScenario(scenario));
                                        })),
                                    ])
                                }),
                            ));
                        }
                        gameplay::LoadScenario::Path(path) => {
                            // Reuse the cached scenario, if possible.
                            if let Some(ref scenario) = app.primary.scenario {
                                if scenario.scenario_name == abstutil::basename(&path) {
                                    self.stage = Some(LoadStage::GotScenario(scenario.clone()));
                                    continue;
                                }
                            }

                            return Transition::Push(FileLoader::<App, Scenario>::new_state(
                                ctx,
                                path,
                                Box::new(|_, _, _, scenario| {
                                    // TODO Handle corrupt files
                                    let scenario = scenario.unwrap();
                                    Transition::Multi(vec![
                                        Transition::Pop,
                                        Transition::ModifyState(Box::new(|state, _, _| {
                                            let loader =
                                                state.downcast_mut::<SandboxLoader>().unwrap();
                                            loader.stage = Some(LoadStage::GotScenario(scenario));
                                        })),
                                    ])
                                }),
                            ));
                        }
                    }
                }
                LoadStage::GotScenario(mut scenario) => {
                    let scenario_name = scenario.scenario_name.clone();
                    ctx.loading_screen("instantiate scenario", |_, mut timer| {
                        app.primary.scenario = Some(scenario.clone());

                        if let GameplayMode::PlayScenario(_, _, ref modifiers) = self.mode {
                            for m in modifiers {
                                scenario = m.apply(&app.primary.map, scenario);
                            }
                        }

                        scenario.instantiate(
                            &mut app.primary.sim,
                            &app.primary.map,
                            &mut app.primary.current_flags.sim_flags.make_rng(),
                            &mut timer,
                        );
                        app.primary
                            .sim
                            .tiny_step(&app.primary.map, &mut app.primary.sim_cb);
                    });

                    self.stage = Some(LoadStage::LoadingPrebaked(scenario_name));
                    continue;
                }
                LoadStage::LoadingPrebaked(scenario_name) => {
                    // Maybe we've already got prebaked data for this map+scenario.
                    if app
                        .has_prebaked()
                        .map(|(m, s)| m == app.primary.map.get_name() && s == &scenario_name)
                        .unwrap_or(false)
                    {
                        self.stage = Some(LoadStage::Finalizing);
                        continue;
                    }

                    return Transition::Push(FileLoader::<App, Analytics>::new_state(
                        ctx,
                        abstio::path_prebaked_results(app.primary.map.get_name(), &scenario_name),
                        Box::new(move |_, _, _, prebaked| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ModifyState(Box::new(move |state, _, _| {
                                    let loader = state.downcast_mut::<SandboxLoader>().unwrap();
                                    loader.stage =
                                        Some(LoadStage::GotPrebaked(scenario_name, prebaked));
                                })),
                            ])
                        }),
                    ));
                }
                LoadStage::GotPrebaked(scenario_name, prebaked) => {
                    match prebaked {
                        Ok(prebaked) => {
                            app.set_prebaked(Some((
                                app.primary.map.get_name().clone(),
                                scenario_name,
                                prebaked,
                            )));
                        }
                        Err(err) => {
                            warn!(
                                "No prebaked simulation results for \"{}\" scenario on {} map. \
                                 This means trip dashboards can't compare current times to any \
                                 kind of baseline: {}",
                                scenario_name,
                                app.primary.map.get_name().describe(),
                                err
                            );
                            app.set_prebaked(None);
                        }
                    }
                    self.stage = Some(LoadStage::Finalizing);
                    continue;
                }
                LoadStage::Finalizing => {
                    let mut gameplay = self.mode.initialize(ctx, app);
                    gameplay.recreate_panels(ctx, app);
                    let sandbox = Box::new(SandboxMode {
                        controls: SandboxControls::new(ctx, app, gameplay.as_ref()),
                        gameplay,
                        gameplay_mode: self.mode.clone(),
                        recalc_unzoomed_agent: None,
                        last_cs: app.opts.color_scheme,
                    });

                    let mut transitions = vec![Transition::Replace(sandbox)];
                    transitions.extend((self.finalize.take().unwrap())(ctx, app));
                    return Transition::Multi(transitions);
                }
            }
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

fn mouseover_unzoomed_agent_circle(ctx: &mut EventCtx, app: &mut App) {
    let cursor = if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
        pt
    } else {
        return;
    };

    for (id, _, _) in app
        .primary
        .agents
        .borrow_mut()
        .calculate_unzoomed_agents(ctx, &app.primary.map, &app.primary.sim, &app.cs)
        .query(
            Circle::new(cursor, Distance::meters(3.0))
                .get_bounds()
                .as_bbox(),
        )
    {
        if let Some(pt) = app
            .primary
            .sim
            .canonical_pt_for_agent(*id, &app.primary.map)
        {
            if Circle::new(pt, unzoomed_agent_radius(id.to_vehicle_type())).contains_pt(cursor) {
                app.primary.current_selection = Some(ID::from_agent(*id));
            }
        }
    }
}

fn is_daytime(app: &App) -> bool {
    let hours = app.primary.sim.time().get_hours() % 24;
    (6..18).contains(&hours)
}

impl SandboxControls {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        gameplay: &dyn gameplay::GameplayState,
    ) -> SandboxControls {
        SandboxControls {
            common: if gameplay.has_common() {
                Some(CommonState::new())
            } else {
                None
            },
            route_preview: if gameplay.can_examine_objects() {
                Some(RoutePreview::new())
            } else {
                None
            },
            tool_panel: if gameplay.has_tool_panel() {
                Some(tool_panel(ctx))
            } else {
                None
            },
            time_panel: if gameplay.has_time_panel() {
                Some(TimePanel::new(ctx, app))
            } else {
                None
            },
            minimap: if gameplay.has_minimap() {
                Some(Minimap::new(ctx, app, MinimapController))
            } else {
                None
            },
        }
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.tool_panel.is_some() {
            self.tool_panel = Some(tool_panel(ctx));
        }
        if let Some(ref mut speed) = self.time_panel {
            speed.recreate_panel(ctx, app);
        }
        if let Some(ref mut minimap) = self.minimap {
            minimap.recreate_panel(ctx, app);
        }
    }
}
