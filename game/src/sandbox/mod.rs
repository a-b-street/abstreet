mod dashboards;
pub mod gameplay;
mod misc_tools;
mod speed;

use self::misc_tools::{RoutePreview, ShowTrafficSignal, TurnExplorer};
use crate::app::App;
use crate::common::{tool_panel, CommonState, ContextualActions, Minimap};
use crate::debug::DebugMode;
use crate::edit::{
    apply_map_edits, can_edit_lane, save_edits_as, EditMode, LaneEditor, StopSignEditor,
    TrafficSignalEditor,
};
use crate::game::{State, Transition, WizardState};
use crate::helpers::{cmp_duration_shorter, ID};
use crate::layer::Layers;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::pregame::main_menu;
use crate::render::AgentColorScheme;
pub use crate::sandbox::gameplay::{TutorialPointer, TutorialState};
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, TextExt, VerticalAlignment, Widget, Wizard,
};
pub use gameplay::spawner::spawn_agents_around;
pub use gameplay::GameplayMode;
use geom::{Duration, Polygon, Statistic, Time};
use map_model::MapEdits;
use sim::{TripMode, VehicleType};
pub use speed::TimeWarpScreen;
pub use speed::{SpeedControls, TimePanel};

pub struct SandboxMode {
    gameplay: Box<dyn gameplay::GameplayState>,
    pub gameplay_mode: GameplayMode,

    pub controls: SandboxControls,
}

pub struct SandboxControls {
    pub common: Option<CommonState>,
    route_preview: Option<RoutePreview>,
    tool_panel: Option<WrappedComposite>,
    time_panel: Option<TimePanel>,
    speed: Option<SpeedControls>,
    agent_meter: Option<AgentMeter>,
    minimap: Option<Minimap>,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> SandboxMode {
        let gameplay = mode.initialize(app, ctx);

        SandboxMode {
            controls: SandboxControls {
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
                    Some(tool_panel(ctx, app))
                } else {
                    None
                },
                time_panel: if gameplay.has_time_panel() {
                    Some(TimePanel::new(ctx, app))
                } else {
                    None
                },
                speed: if gameplay.has_speed() {
                    Some(SpeedControls::new(ctx, app))
                } else {
                    None
                },
                agent_meter: if let Some(show_score) = gameplay.get_agent_meter_params() {
                    Some(AgentMeter::new(ctx, app, show_score))
                } else {
                    None
                },
                minimap: if gameplay.has_minimap() {
                    Some(Minimap::new(ctx, app))
                } else {
                    None
                },
            },
            gameplay,
            gameplay_mode: mode,
        }
    }

    // Just for Warping
    pub fn contextual_actions(&self) -> Actions {
        Actions {
            is_paused: self
                .controls
                .speed
                .as_ref()
                .map(|s| s.is_paused())
                .unwrap_or(true),
            can_interact: self.gameplay.can_examine_objects(),
            gameplay: self.gameplay_mode.clone(),
        }
    }
}

impl State for SandboxMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // Do this before gameplay
        if self.gameplay.can_move_canvas() {
            ctx.canvas_movement();
        }

        let (maybe_t, exit) = self.gameplay.event(ctx, app, &mut self.controls);
        if let Some(t) = maybe_t {
            return t;
        }
        if exit {
            return Transition::Push(WizardState::new(Box::new(exit_sandbox)));
        }

        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        // Order here is pretty arbitrary
        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx, app)));
        }

        if let Some(ref mut m) = self.controls.minimap {
            if let Some(t) = m.event(app, ctx) {
                return t;
            }
            if let Some(t) = Layers::update(ctx, app, &m.composite) {
                return t;
            }
        }

        if let Some(ref mut s) = self.controls.speed {
            if let Some(t) = s.event(ctx, app, Some(&self.gameplay_mode)) {
                return t;
            }
        }

        if let Some(ref mut r) = self.controls.route_preview {
            if let Some(t) = r.event(ctx, app) {
                return t;
            }
        }

        // Fragile ordering. Let this work before tool_panel, so Key::Escape from the info panel
        // beats the one to quit. And let speed update the sim before we update the info panel.
        let mut actions = self.contextual_actions();
        if let Some(ref mut c) = self.controls.common {
            if let Some(t) = c.event(ctx, app, &mut actions) {
                return t;
            }
        }

        if let Some(ref mut tp) = self.controls.time_panel {
            tp.event(ctx, app);
        }

        if let Some(ref mut tp) = self.controls.tool_panel {
            match tp.event(ctx, app) {
                Some(WrappedOutcome::Transition(t)) => {
                    return t;
                }
                Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                    "back" => {
                        return Transition::Push(WizardState::new(Box::new(exit_sandbox)));
                    }
                    _ => unreachable!(),
                },
                None => {}
            }
        }
        if let Some(ref mut am) = self.controls.agent_meter {
            if let Some(t) = am.event(ctx, app) {
                return t;
            }
        }

        if self
            .controls
            .speed
            .as_ref()
            .map(|s| s.is_paused())
            .unwrap_or(true)
        {
            Transition::Keep
        } else {
            Transition::KeepWithMode(EventLoopMode::Animation)
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.layer.draw(g);

        if let Some(ref c) = self.controls.common {
            c.draw(g, app);
        } else {
            CommonState::draw_osd(g, app, &None);
        }
        if let Some(ref tp) = self.controls.tool_panel {
            tp.draw(g);
        }
        if let Some(ref s) = self.controls.speed {
            s.draw(g);
        }
        if let Some(ref tp) = self.controls.time_panel {
            tp.draw(g);
        }
        if let Some(ref am) = self.controls.agent_meter {
            am.draw(g);
        }
        if let Some(ref m) = self.controls.minimap {
            m.draw(g, app);
        }
        if let Some(ref r) = self.controls.route_preview {
            r.draw(g);
        }

        self.gameplay.draw(g, app);
    }

    fn on_suspend(&mut self, ctx: &mut EventCtx, app: &mut App) {
        if let Some(ref mut s) = self.controls.speed {
            s.pause(ctx, app);
        }
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        app.layer = Layers::Inactive;
        app.agent_cs = AgentColorScheme::new(&app.cs);
    }
}

fn exit_sandbox(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let unsaved = app.primary.map.get_edits().edits_name == "untitled edits"
        && !app.primary.map.get_edits().commands.is_empty();
    let (resp, _) = wizard.choose("Are you ready to leave this mode?", || {
        let mut choices = Vec::new();
        choices.push(Choice::new("keep playing", ()));
        if unsaved {
            choices.push(Choice::new("save edits first", ()));
        }
        choices.push(Choice::new("quit to main screen", ()).key(Key::Q));
        choices
    })?;
    if resp == "keep playing" {
        return Some(Transition::Pop);
    }
    let map_name = app.primary.map.get_name().clone();
    if resp == "save edits and quit" {
        save_edits_as(&mut wizard, app)?;
    }
    ctx.loading_screen("reset map and sim", |ctx, mut timer| {
        if app.primary.map.get_edits().edits_name != "untitled edits"
            || !app.primary.map.get_edits().commands.is_empty()
        {
            apply_map_edits(ctx, app, MapEdits::new(&map_name));
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
        }
        app.primary.clear_sim();
        app.set_prebaked(None);
    });
    ctx.canvas.save_camera_state(&map_name);
    Some(Transition::Clear(vec![main_menu(ctx, app)]))
}

#[derive(Clone, Copy)]
pub struct ScoreCard {
    pub stat: Statistic,
    pub goal: Duration,
}

pub struct AgentMeter {
    time: Time,
    pub composite: Composite,
    pub show_score: Option<ScoreCard>,
}

impl AgentMeter {
    pub fn new(ctx: &mut EventCtx, app: &App, show_score: Option<ScoreCard>) -> AgentMeter {
        use abstutil::prettyprint_usize;

        let (finished, unfinished, by_mode) = app.primary.sim.num_trips();

        let mut rows = vec![
            "Active agents".draw_text(ctx),
            Widget::row(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/meters/pedestrian.svg")
                        .margin_right(5),
                    prettyprint_usize(by_mode[&TripMode::Walk]).draw_text(ctx),
                ]),
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/meters/bike.svg").margin_right(5),
                    prettyprint_usize(by_mode[&TripMode::Bike]).draw_text(ctx),
                ]),
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/meters/car.svg").margin_right(5),
                    prettyprint_usize(by_mode[&TripMode::Drive]).draw_text(ctx),
                ]),
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/meters/bus.svg").margin_right(5),
                    prettyprint_usize(by_mode[&TripMode::Transit]).draw_text(ctx),
                ]),
            ])
            .centered(),
            // Separator
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
                )]),
            )
            .margin(15)
            .centered_horiz(),
            Widget::row(vec![
                {
                    let mut txt = Text::new();
                    let pct = if unfinished == 0 {
                        100.0
                    } else {
                        100.0 * (finished as f64) / ((finished + unfinished) as f64)
                    };
                    txt.add(Line(format!(
                        "Finished trips: {} ({}%)",
                        prettyprint_usize(finished),
                        pct as usize
                    )));
                    txt.draw(ctx)
                },
                Btn::svg_def("../data/system/assets/meters/trip_histogram.svg")
                    .build(ctx, "more data", hotkey(Key::Q))
                    .align_right(),
            ]),
        ];
        // TODO Slight hack. If we're jumping right into a tutorial and don't have the prebaked
        // stuff loaded yet, just skip a tick.
        if app.has_prebaked().is_some() {
            if let Some(ScoreCard { stat, goal }) = show_score {
                // Separator
                rows.push(
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::from(vec![(
                            Color::WHITE,
                            Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
                        )]),
                    )
                    .margin(15)
                    .centered_horiz(),
                );

                let (after, _, _) = app
                    .primary
                    .sim
                    .get_analytics()
                    .trip_times(app.primary.sim.time());
                let (before, _, _) = app.prebaked().trip_times(app.primary.sim.time());
                let mut txt = Text::from(Line(format!("{} trip time: ", stat)).secondary());
                if after.count() > 0 && before.count() > 0 {
                    txt.append_all(cmp_duration_shorter(
                        after.select(stat),
                        before.select(stat),
                    ));
                } else {
                    txt.append(Line("same"));
                }
                txt.add(Line(format!("Goal: {} faster", goal)).secondary());
                rows.push(txt.draw(ctx));
            }
        }

        let composite = Composite::new(Widget::col(rows).bg(app.cs.panel_bg).padding(20))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx);

        AgentMeter {
            time: app.primary.sim.time(),
            composite,
            show_score,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        if self.time != app.primary.sim.time() {
            *self = AgentMeter::new(ctx, app, self.show_score);
            return self.event(ctx, app);
        }
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "more data" => {
                    return Some(Transition::Push(dashboards::TripTable::new(ctx, app)));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
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
            match id.clone() {
                ID::Intersection(i) => {
                    if app.primary.map.get_i(i).is_traffic_signal() {
                        actions.push((Key::F, "explore traffic signal details".to_string()));
                        actions.push((Key::C, "show current demand".to_string()));
                        actions.push((Key::E, "edit traffic signal".to_string()));
                    }
                    if app.primary.map.get_i(i).is_stop_sign() {
                        actions.push((Key::E, "edit stop sign".to_string()));
                    }
                }
                ID::Lane(l) => {
                    if !app.primary.map.get_turns_from_lane(l).is_empty() {
                        actions.push((Key::Z, "explore turns from this lane".to_string()));
                    }
                    if can_edit_lane(&self.gameplay, l, app) {
                        actions.push((Key::E, "edit lane".to_string()));
                    }
                }
                ID::Car(c) => {
                    if c.1 == VehicleType::Bus {
                        let route = app.primary.sim.bus_route_id(c).unwrap();
                        match app.layer {
                            Layers::BusRoute(_, r, _) if r == route => {}
                            _ => {
                                actions.push((Key::R, "show route".to_string()));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        actions.extend(self.gameplay.actions(app, id));
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
            (ID::Intersection(i), "explore traffic signal details") => {
                Transition::Push(ShowTrafficSignal::new(ctx, app, i))
            }
            (ID::Intersection(i), "show current demand") => {
                app.layer = crate::layer::traffic::intersection_demand(ctx, app, i);
                Transition::Keep
            }
            (ID::Intersection(i), "edit traffic signal") => {
                let edit = EditMode::new(ctx, app, self.gameplay.clone());
                let sim_copy = edit.suspended_sim.clone();
                Transition::PushTwice(
                    Box::new(edit),
                    Box::new(TrafficSignalEditor::new(i, ctx, app, sim_copy)),
                )
            }
            (ID::Intersection(i), "edit stop sign") => {
                let edit = EditMode::new(ctx, app, self.gameplay.clone());
                let sim_copy = edit.suspended_sim.clone();
                Transition::PushTwice(
                    Box::new(edit),
                    Box::new(StopSignEditor::new(i, ctx, app, sim_copy)),
                )
            }
            (ID::Lane(l), "explore turns from this lane") => {
                Transition::Push(TurnExplorer::new(ctx, app, l))
            }
            (ID::Lane(l), "edit lane") => Transition::PushTwice(
                Box::new(EditMode::new(ctx, app, self.gameplay.clone())),
                Box::new(LaneEditor::new(ctx, app, l, self.gameplay.clone())),
            ),
            (ID::Car(c), "show route") => {
                *close_panel = false;
                app.layer = crate::layer::bus::ShowBusRoute::new(
                    ctx,
                    app,
                    app.primary.sim.bus_route_id(c).unwrap(),
                );
                Transition::Keep
            }
            (_, "follow") => {
                *close_panel = false;
                Transition::KeepWithData(Box::new(|state, app, ctx| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let speed = mode.controls.speed.as_mut().unwrap();
                    assert!(speed.is_paused());
                    speed.resume_realtime(ctx, app);
                }))
            }
            (_, "unfollow") => {
                *close_panel = false;
                Transition::KeepWithData(Box::new(|state, app, ctx| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let speed = mode.controls.speed.as_mut().unwrap();
                    assert!(!speed.is_paused());
                    speed.pause(ctx, app);
                }))
            }
            (id, action) => self
                .gameplay
                .execute(ctx, app, id, action.to_string(), close_panel),
        }
    }

    fn is_paused(&self) -> bool {
        self.is_paused
    }
}
