mod dashboards;
pub mod gameplay;
mod misc_tools;
mod speed;
mod uber_turns;

use self::misc_tools::{RoutePreview, ShowTrafficSignal, TurnExplorer};
use crate::app::App;
use crate::common::{tool_panel, CommonState, ContextualActions, IsochroneViewer, Minimap};
use crate::debug::DebugMode;
use crate::edit::{
    apply_map_edits, can_edit_lane, EditMode, LaneEditor, SaveEdits, StopSignEditor,
    TrafficSignalEditor,
};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::layer::PickLayer;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::pregame::MainMenu;
use crate::render::UnzoomedAgents;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, Text, TextExt, UpdateType, VerticalAlignment, Widget, Wizard,
};
pub use gameplay::{spawn_agents_around, GameplayMode, TutorialPointer, TutorialState};
use geom::{Polygon, Time};
use map_model::MapEdits;
use sim::AgentType;
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
    pub agent_meter: Option<AgentMeter>,
    minimap: Option<Minimap>,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> SandboxMode {
        app.primary.clear_sim();
        let gameplay = mode.initialize(ctx, app);

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
                    Some(tool_panel(ctx))
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
                agent_meter: if gameplay.has_agent_meter() {
                    Some(AgentMeter::new(ctx, app))
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

        if let Some(t) = self.gameplay.event(ctx, app, &mut self.controls) {
            return t;
        }

        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        // Order here is pretty arbitrary
        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx)));
        }

        if let Some(ref mut m) = self.controls.minimap {
            if let Some(t) = m.event(ctx, app) {
                return t;
            }
            if let Some(t) = PickLayer::update(ctx, app, &m.composite) {
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
                        return maybe_exit_sandbox();
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
            ctx.request_update(UpdateType::Game);
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if let Some(ref l) = app.layer {
            l.draw(g, app);
        }

        if let Some(ref c) = self.controls.common {
            c.draw(g, app);
        } else {
            CommonState::draw_osd(g, app);
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

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        app.layer = None;
        app.unzoomed_agents = UnzoomedAgents::new(&app.cs);
        self.gameplay.on_destroy(app);
    }
}

pub fn maybe_exit_sandbox() -> Transition {
    Transition::Push(WizardState::new(Box::new(exit_sandbox)))
}

fn exit_sandbox(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let (resp, _) = wiz
        .wrap(ctx)
        .choose("Are you ready to leave this mode?", || {
            vec![
                Choice::new("keep playing", ()),
                Choice::new("quit to main screen", ()).key(Key::Q),
            ]
        })?;
    if resp == "keep playing" {
        return Some(Transition::Pop);
    }

    ctx.canvas.save_camera_state(app.primary.map.get_name());
    if app.primary.map.unsaved_edits() {
        return Some(Transition::PushTwice(
            Box::new(BackToMainMenu),
            SaveEdits::new(
                ctx,
                app,
                "Do you want to save your edits first?",
                true,
                None,
            ),
        ));
    }
    Some(Transition::Replace(Box::new(BackToMainMenu)))
}

struct BackToMainMenu;

impl State for BackToMainMenu {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.loading_screen("reset map and sim", |ctx, mut timer| {
            // Always safe to do this
            apply_map_edits(ctx, app, MapEdits::new());
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);

            app.primary.clear_sim();
            app.set_prebaked(None);
        });
        Transition::Clear(vec![MainMenu::new(ctx, app)])
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

pub struct AgentMeter {
    time: Time,
    pub composite: Composite,
}

impl AgentMeter {
    pub fn new(ctx: &mut EventCtx, app: &App) -> AgentMeter {
        use abstutil::prettyprint_usize;

        let (finished, unfinished) = app.primary.sim.num_trips();
        let by_type = app.primary.sim.num_agents();

        let rows = vec![
            "Active trips".draw_text(ctx),
            Widget::custom_row(vec![
                Widget::custom_row(vec![
                    Widget::draw_svg(ctx, "system/assets/meters/pedestrian.svg").margin_right(5),
                    prettyprint_usize(by_type[&AgentType::Pedestrian]).draw_text(ctx),
                ]),
                Widget::custom_row(vec![
                    Widget::draw_svg(ctx, "system/assets/meters/bike.svg").margin_right(5),
                    prettyprint_usize(by_type[&AgentType::Bike]).draw_text(ctx),
                ]),
                Widget::custom_row(vec![
                    Widget::draw_svg(ctx, "system/assets/meters/car.svg").margin_right(5),
                    prettyprint_usize(by_type[&AgentType::Car]).draw_text(ctx),
                ]),
                Widget::custom_row(vec![
                    Widget::draw_svg(ctx, "system/assets/meters/bus.svg").margin_right(5),
                    prettyprint_usize(by_type[&AgentType::Bus] + by_type[&AgentType::Train])
                        .draw_text(ctx),
                ]),
                Widget::custom_row(vec![
                    Widget::draw_svg(ctx, "system/assets/meters/passenger.svg").margin_right(5),
                    prettyprint_usize(by_type[&AgentType::TransitRider]).draw_text(ctx),
                ]),
            ])
            .centered(),
            // Separator
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
                )]),
            )
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
                Btn::svg_def("system/assets/meters/trip_histogram.svg")
                    .build(ctx, "more data", hotkey(Key::Q))
                    .align_right(),
            ]),
        ];

        let composite = Composite::new(Widget::col(rows))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx);

        AgentMeter {
            time: app.primary.sim.time(),
            composite,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        if self.time != app.primary.sim.time() {
            *self = AgentMeter::new(ctx, app);
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
                        actions.push((Key::E, "edit traffic signal".to_string()));
                    }
                    if app.primary.map.get_i(i).is_stop_sign()
                        && self.gameplay.can_edit_stop_signs()
                    {
                        actions.push((Key::E, "edit stop sign".to_string()));
                    }
                    if app.opts.dev {
                        actions.push((Key::U, "explore uber-turns".to_string()));
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
                ID::Building(_) => {
                    if app.opts.dev {
                        actions.push((Key::I, "explore isochrone from here".to_string()));
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
            (ID::Intersection(i), "edit traffic signal") => Transition::PushTwice(
                Box::new(EditMode::new(ctx, app, self.gameplay.clone())),
                Box::new(TrafficSignalEditor::new(ctx, app, i, self.gameplay.clone())),
            ),
            (ID::Intersection(i), "edit stop sign") => Transition::PushTwice(
                Box::new(EditMode::new(ctx, app, self.gameplay.clone())),
                Box::new(StopSignEditor::new(ctx, app, i, self.gameplay.clone())),
            ),
            (ID::Intersection(i), "explore uber-turns") => {
                Transition::Push(uber_turns::UberTurnPicker::new(ctx, app, i))
            }
            (ID::Lane(l), "explore turns from this lane") => {
                Transition::Push(TurnExplorer::new(ctx, app, l))
            }
            (ID::Lane(l), "edit lane") => Transition::PushTwice(
                Box::new(EditMode::new(ctx, app, self.gameplay.clone())),
                Box::new(LaneEditor::new(ctx, app, l, self.gameplay.clone())),
            ),
            (ID::Building(b), "explore isochrone from here") => {
                Transition::Push(IsochroneViewer::new(ctx, app, b))
            }
            (_, "follow (run the simulation)") => {
                *close_panel = false;
                Transition::KeepWithData(Box::new(|state, ctx, app| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let speed = mode.controls.speed.as_mut().unwrap();
                    assert!(speed.is_paused());
                    speed.resume_realtime(ctx, app);
                }))
            }
            (_, "unfollow (pause the simulation)") => {
                *close_panel = false;
                Transition::KeepWithData(Box::new(|state, ctx, app| {
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
