pub use gameplay::{spawn_agents_around, GameplayMode, TutorialPointer, TutorialState};
use maplit::btreeset;
pub use speed::{SpeedControls, TimePanel};
pub use time_warp::TimeWarpScreen;

use geom::Time;
use sim::AgentType;
use widgetry::{
    lctrl, Btn, Choice, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Text,
    TextExt, UpdateType, VerticalAlignment, Widget,
};

use self::misc_tools::{RoutePreview, TurnExplorer};
use crate::app::App;
use crate::common::{tool_panel, CommonState, ContextualActions, IsochroneViewer, Minimap};
use crate::debug::DebugMode;
use crate::edit::{
    apply_map_edits, can_edit_lane, EditMode, LaneEditor, SaveEdits, StopSignEditor,
    TrafficSignalEditor,
};
use crate::game::{ChooseSomething, State, Transition};
use crate::helpers::ID;
use crate::layer::PickLayer;
use crate::options::OptionsPanel;
use crate::pregame::MainMenu;
use crate::render::UnzoomedAgents;

pub mod dashboards;
pub mod gameplay;
mod misc_tools;
mod speed;
mod time_warp;
mod uber_turns;

pub struct SandboxMode {
    gameplay: Box<dyn gameplay::GameplayState>,
    pub gameplay_mode: GameplayMode,

    pub controls: SandboxControls,
}

pub struct SandboxControls {
    pub common: Option<CommonState>,
    route_preview: Option<RoutePreview>,
    tool_panel: Option<Panel>,
    time_panel: Option<TimePanel>,
    speed: Option<SpeedControls>,
    pub agent_meter: Option<AgentMeter>,
    minimap: Option<Minimap>,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> Box<dyn State> {
        app.primary.clear_sim();
        let gameplay = mode.initialize(ctx, app);

        Box::new(SandboxMode {
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
        })
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
            return Transition::Push(DebugMode::new(ctx));
        }

        if let Some(ref mut m) = self.controls.minimap {
            if let Some(t) = m.event(ctx, app) {
                return t;
            }
            if let Some(t) = PickLayer::update(ctx, app, &m.panel) {
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
        if let Some(ref mut c) = self.controls.common {
            if let Some(t) = c.event(ctx, app, &mut actions) {
                return t;
            }
        }

        if let Some(ref mut tp) = self.controls.time_panel {
            tp.event(ctx, app);
        }

        if let Some(ref mut tp) = self.controls.tool_panel {
            match tp.event(ctx) {
                Outcome::Clicked(x) => match x.as_ref() {
                    "back" => {
                        return maybe_exit_sandbox(ctx);
                    }
                    "settings" => {
                        return Transition::Push(OptionsPanel::new(ctx, app));
                    }
                    _ => unreachable!(),
                },
                _ => {}
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

pub fn maybe_exit_sandbox(ctx: &mut EventCtx) -> Transition {
    Transition::Push(ChooseSomething::new(
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

            ctx.canvas.save_camera_state(app.primary.map.get_name());
            if app.primary.map.unsaved_edits() {
                return Transition::Multi(vec![
                    Transition::Push(Box::new(BackToMainMenu)),
                    Transition::Push(SaveEdits::new(
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

impl State for BackToMainMenu {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.loading_screen("reset map and sim", |ctx, mut timer| {
            // Always safe to do this
            apply_map_edits(ctx, app, app.primary.map.new_edits());
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
    pub panel: Panel,
}

impl AgentMeter {
    pub fn new(ctx: &mut EventCtx, app: &App) -> AgentMeter {
        use abstutil::prettyprint_usize;

        let (finished, unfinished) = app.primary.sim.num_trips();
        let by_type = app.primary.sim.num_agents();

        let mut row = Vec::new();
        for (agent_type, name) in vec![
            (AgentType::Pedestrian, "pedestrian"),
            (AgentType::Bike, "bike"),
            (AgentType::Car, "car"),
        ] {
            let n = prettyprint_usize(by_type.get(agent_type));
            row.push(Widget::custom_row(vec![
                Widget::draw_svg_with_tooltip(
                    ctx,
                    format!("system/assets/meters/{}.svg", name),
                    Text::from(Line(format!("{} {}", n, agent_type.plural_noun()))),
                )
                .margin_right(5),
                n.draw_text(ctx),
            ]));
        }
        row.push(Widget::custom_row(vec![
            Widget::draw_svg_with_tooltip(
                ctx,
                "system/assets/meters/bus.svg",
                Text::from_multiline(vec![
                    Line(format!(
                        "{} public transit passengers",
                        prettyprint_usize(by_type.get(AgentType::TransitRider))
                    )),
                    Line(format!(
                        "{} buses",
                        prettyprint_usize(by_type.get(AgentType::Bus))
                    )),
                    Line(format!(
                        "{} trains",
                        prettyprint_usize(by_type.get(AgentType::Train))
                    )),
                ]),
            )
            .margin_right(5),
            prettyprint_usize(by_type.get(AgentType::TransitRider)).draw_text(ctx),
        ]));

        let rows = vec![
            "Active trips".draw_text(ctx),
            Widget::custom_row(row).centered(),
            Widget::horiz_separator(ctx, 0.2),
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
                    .build(ctx, "more data", Key::Q)
                    .align_right(),
            ]),
        ];

        let panel = Panel::new(Widget::col(rows))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx);

        AgentMeter {
            time: app.primary.sim.time(),
            panel,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        if self.time != app.primary.sim.time() {
            *self = AgentMeter::new(ctx, app);
            return self.event(ctx, app);
        }
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "more data" => {
                    return Some(Transition::Push(dashboards::FinishedTripTable::new(
                        ctx, app,
                    )));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
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
                Transition::Push(EditMode::new(ctx, app, self.gameplay.clone())),
                Transition::Push(TrafficSignalEditor::new(
                    ctx,
                    app,
                    btreeset! {i},
                    self.gameplay.clone(),
                )),
            ]),
            (ID::Intersection(i), "edit stop sign") => Transition::Multi(vec![
                Transition::Push(EditMode::new(ctx, app, self.gameplay.clone())),
                Transition::Push(StopSignEditor::new(ctx, app, i, self.gameplay.clone())),
            ]),
            (ID::Intersection(i), "explore uber-turns") => {
                Transition::Push(uber_turns::UberTurnPicker::new(ctx, app, i))
            }
            (ID::Lane(l), "explore turns from this lane") => {
                Transition::Push(TurnExplorer::new(ctx, app, l))
            }
            (ID::Lane(l), "edit lane") => Transition::Multi(vec![
                Transition::Push(EditMode::new(ctx, app, self.gameplay.clone())),
                Transition::Push(LaneEditor::new(ctx, app, l, self.gameplay.clone())),
            ]),
            (ID::Building(b), "explore isochrone from here") => {
                Transition::Push(IsochroneViewer::new(ctx, app, b))
            }
            (_, "follow (run the simulation)") => {
                *close_panel = false;
                Transition::ModifyState(Box::new(|state, ctx, app| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let speed = mode.controls.speed.as_mut().unwrap();
                    assert!(speed.is_paused());
                    speed.resume_realtime(ctx, app);
                }))
            }
            (_, "unfollow (pause the simulation)") => {
                *close_panel = false;
                Transition::ModifyState(Box::new(|state, ctx, app| {
                    let mode = state.downcast_mut::<SandboxMode>().unwrap();
                    let speed = mode.controls.speed.as_mut().unwrap();
                    assert!(!speed.is_paused());
                    speed.pause(ctx, app);
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
