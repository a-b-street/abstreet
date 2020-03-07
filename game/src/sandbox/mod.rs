mod dashboards;
mod gameplay;
mod speed;

use crate::app::{App, ShowEverything};
use crate::colors;
use crate::common::{tool_panel, CommonState, Minimap, Overlays, ShowBusRoute};
use crate::debug::DebugMode;
use crate::edit::{
    apply_map_edits, can_edit_lane, save_edits_as, EditMode, LaneEditor, StopSignEditor,
    TrafficSignalEditor,
};
use crate::game::{DrawBaselayer, State, Transition, WizardState};
use crate::helpers::{cmp_duration_shorter, ID};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::pregame::main_menu;
use crate::render::{AgentColorScheme, DrawOptions};
pub use crate::sandbox::gameplay::{TutorialPointer, TutorialState};
use ezgui::{
    hotkey, lctrl, Choice, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Text, VerticalAlignment, Wizard,
};
pub use gameplay::spawner::spawn_agents_around;
pub use gameplay::GameplayMode;
use geom::{Duration, Polygon, Statistic, Time};
use map_model::MapEdits;
use sim::TripMode;
pub use speed::{SpeedControls, TimePanel};

pub struct SandboxMode {
    gameplay: Box<dyn gameplay::GameplayState>,
    gameplay_mode: GameplayMode,

    pub controls: SandboxControls,
}

pub struct SandboxControls {
    pub common: Option<CommonState>,
    tool_panel: Option<WrappedComposite>,
    time_panel: Option<TimePanel>,
    pub speed: Option<SpeedControls>,
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
                    Some(SpeedControls::new(ctx))
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

    fn examine_objects(&self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::D).unwrap()) {
            return Some(Transition::Push(Box::new(DebugMode::new(ctx))));
        }

        if let Some(ID::Building(b)) = app.primary.current_selection {
            let cars = app
                .primary
                .sim
                .get_offstreet_parked_cars(b)
                .into_iter()
                .map(|p| p.vehicle.id)
                .collect::<Vec<_>>();
            if !cars.is_empty()
                && app.per_obj.action(
                    ctx,
                    Key::P,
                    format!("examine {} cars parked here", cars.len()),
                )
            {
                return Some(Transition::Push(WizardState::new(Box::new(
                    move |wiz, ctx, _| {
                        let _id = wiz.wrap(ctx).choose("Examine which car?", || {
                            cars.iter()
                                .map(|c| Choice::new(c.to_string(), *c))
                                .collect()
                        })?;
                        Some(Transition::Pop)
                    },
                ))));
            }
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if app.primary.map.get_i(i).is_traffic_signal()
                && app.per_obj.action(ctx, Key::C, "show current demand")
            {
                app.overlay = Overlays::intersection_demand(i, ctx, app);
            }

            if app.primary.map.get_i(i).is_traffic_signal()
                && app.per_obj.action(ctx, Key::E, "edit traffic signal")
            {
                let edit = EditMode::new(ctx, app, self.gameplay_mode.clone());
                let sim_copy = edit.suspended_sim.clone();
                return Some(Transition::PushTwice(
                    Box::new(edit),
                    Box::new(TrafficSignalEditor::new(i, ctx, app, sim_copy)),
                ));
            }
            if app.primary.map.get_i(i).is_stop_sign()
                && app.per_obj.action(ctx, Key::E, "edit stop sign")
            {
                let edit = EditMode::new(ctx, app, self.gameplay_mode.clone());
                let sim_copy = edit.suspended_sim.clone();
                return Some(Transition::PushTwice(
                    Box::new(edit),
                    Box::new(StopSignEditor::new(i, ctx, app, sim_copy)),
                ));
            }
        }
        if let Some(ID::Lane(l)) = app.primary.current_selection {
            if can_edit_lane(&self.gameplay_mode, l, app)
                && app.per_obj.action(ctx, Key::E, "edit lane")
            {
                return Some(Transition::PushTwice(
                    Box::new(EditMode::new(ctx, app, self.gameplay_mode.clone())),
                    Box::new(LaneEditor::new(l, ctx, app)),
                ));
            }
        }
        if let Some(ID::BusStop(bs)) = app.primary.current_selection {
            let routes = app.primary.map.get_routes_serving_stop(bs);
            if app.per_obj.action(ctx, Key::E, "explore bus route") {
                return Some(Transition::Push(ShowBusRoute::make_route_picker(
                    routes.into_iter().map(|r| r.id).collect(),
                    true,
                )));
            }
        }
        if let Some(ID::Car(c)) = app.primary.current_selection {
            if let Some(r) = app.primary.sim.bus_route_id(c) {
                if app.per_obj.action(ctx, Key::E, "explore bus route") {
                    return Some(Transition::Push(ShowBusRoute::make_route_picker(
                        vec![r],
                        true,
                    )));
                }
            }
        }

        None
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
        if let Some(ref mut m) = self.controls.minimap {
            if let Some(t) = m.event(app, ctx) {
                return t;
            }
            if let Some(t) = Overlays::update(ctx, app, &m.composite) {
                return t;
            }
        }

        if self.gameplay.can_examine_objects() {
            if let Some(t) = self.examine_objects(ctx, app) {
                return t;
            }
        }

        if let Some(ref mut s) = self.controls.speed {
            if let Some(t) = s.event(ctx, app, Some(&self.gameplay_mode)) {
                return t;
            }
        }

        // Fragile ordering. Don't call this before all the per_obj actions have been called. But
        // also let this work before tool_panel, so Key::Escape from the info panel beats the one
        // to quit. And let speed update the sim before we update the info panel.
        if let Some(ref mut c) = self.controls.common {
            if let Some(t) = c.event(ctx, app, self.controls.speed.as_mut()) {
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

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.draw(
            g,
            self.controls
                .common
                .as_ref()
                .map(|c| c.draw_options(app))
                .unwrap_or_else(DrawOptions::new),
            &app.primary.sim,
            &ShowEverything::new(),
        );
        app.overlay.draw(g);

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

        self.gameplay.draw(g, app);
    }

    fn on_suspend(&mut self, ctx: &mut EventCtx, _: &mut App) {
        if let Some(ref mut s) = self.controls.speed {
            s.pause(ctx);
        }
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        app.overlay = Overlays::Inactive;
        app.agent_cs = AgentColorScheme::default(&app.cs);
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
    let map_name = app.primary.map.get_name().to_string();
    if resp == "save edits and quit" {
        save_edits_as(&mut wizard, app)?;
    }
    ctx.loading_screen("reset map and sim", |ctx, mut timer| {
        if app.primary.map.get_edits().edits_name != "untitled edits"
            || !app.primary.map.get_edits().commands.is_empty()
        {
            apply_map_edits(ctx, app, MapEdits::new(map_name));
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
        }
        app.primary.clear_sim();
        app.set_prebaked(None);
    });
    ctx.canvas.save_camera_state(app.primary.map.get_name());
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
            ManagedWidget::draw_text(ctx, Text::from(Line("Active agents"))),
            ManagedWidget::row(vec![
                ManagedWidget::draw_svg(ctx, "../data/system/assets/meters/pedestrian.svg"),
                ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(prettyprint_usize(by_mode[&TripMode::Walk]))),
                ),
                ManagedWidget::draw_svg(ctx, "../data/system/assets/meters/bike.svg"),
                ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(prettyprint_usize(by_mode[&TripMode::Bike]))),
                ),
                ManagedWidget::draw_svg(ctx, "../data/system/assets/meters/car.svg"),
                ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(prettyprint_usize(by_mode[&TripMode::Drive]))),
                ),
                ManagedWidget::draw_svg(ctx, "../data/system/assets/meters/bus.svg"),
                ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(prettyprint_usize(by_mode[&TripMode::Transit]))),
                ),
            ])
            .centered(),
            // Separator
            ManagedWidget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
                )]),
            )
            .margin(15)
            .centered_horiz(),
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
                ManagedWidget::draw_text(ctx, txt)
            },
            {
                ManagedWidget::row(vec![
                    WrappedComposite::text_bg_button(ctx, "more data", hotkey(Key::Q)),
                    if app.has_prebaked().is_some() {
                        WrappedComposite::svg_button(
                            ctx,
                            "../data/system/assets/meters/trip_histogram.svg",
                            "compare trips to baseline",
                            None,
                        )
                        .align_right()
                    } else {
                        ManagedWidget::nothing()
                    },
                ])
                .centered()
            },
        ];
        // TODO Slight hack. If we're jumping right into a tutorial and don't have the prebaked
        // stuff loaded yet, just skip a tick.
        if app.has_prebaked().is_some() {
            if let Some(ScoreCard { stat, goal }) = show_score {
                // Separator
                rows.push(
                    ManagedWidget::draw_batch(
                        ctx,
                        GeomBatch::from(vec![(
                            Color::WHITE,
                            Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
                        )]),
                    )
                    .margin(15)
                    .centered_horiz(),
                );

                let (now, _, _) = app
                    .primary
                    .sim
                    .get_analytics()
                    .trip_times(app.primary.sim.time());
                let (baseline, _, _) = app.prebaked().trip_times(app.primary.sim.time());
                let mut txt = Text::from(Line(format!("{} trip time: ", stat)).size(20));
                if now.count() > 0 && baseline.count() > 0 {
                    txt.append_all(cmp_duration_shorter(
                        now.select(stat),
                        baseline.select(stat),
                    ));
                } else {
                    txt.append(Line("same as baseline"));
                }
                txt.add(Line(format!("Goal: {} faster", goal)).size(20));
                rows.push(ManagedWidget::draw_text(ctx, txt));
            }
        }

        let composite = Composite::new(ManagedWidget::col(rows).bg(colors::PANEL_BG).padding(20))
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
                    return Some(Transition::Push(dashboards::make(
                        ctx,
                        app,
                        dashboards::Tab::TripsSummary,
                    )));
                }
                "compare trips to baseline" => {
                    app.overlay = Overlays::trips_histogram(ctx, app);
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
