use crate::app::{App, ShowEverything};
use crate::edit::traffic_signals::{draw_selected_group, make_top_panel, PreviewTrafficSignal};
use crate::game::{ChooseSomething, DrawBaselayer, State, Transition};
use crate::options::TrafficSignalStyle;
use crate::render::{draw_signal_phase, DrawOptions, DrawTurnGroup};
use crate::sandbox::{spawn_agents_around, GameplayMode};
use abstutil::Timer;
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Spinner, TextExt, VerticalAlignment, Widget,
};
use geom::{Bounds, Distance, Duration, Polygon};
use map_model::{
    ControlTrafficSignal, IntersectionID, Phase, PhaseType, TurnGroupID, TurnPriority,
};
use std::collections::BTreeSet;

pub struct NewTrafficSignalEditor {
    side_panel: Composite,
    top_panel: Composite,

    gameplay: GameplayMode,
    members: BTreeSet<IntersectionID>,
    current_phase: usize,

    groups: Vec<DrawTurnGroup>,
    // And the next priority to toggle to
    group_selected: Option<(TurnGroupID, Option<TurnPriority>)>,

    // The first is the original
    command_stack: Vec<BundleEdits>,
    redo_stack: Vec<BundleEdits>,

    fade_irrelevant: Drawable,
}

// For every member intersection, the full state of that signal
#[derive(Clone)]
struct BundleEdits {
    signals: Vec<ControlTrafficSignal>,
}

impl NewTrafficSignalEditor {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        members: BTreeSet<IntersectionID>,
        gameplay: GameplayMode,
    ) -> Box<dyn State> {
        let map = &app.primary.map;
        app.primary.current_selection = None;

        let fade_area = {
            let mut holes = Vec::new();
            for i in &members {
                let i = map.get_i(*i);
                holes.push(i.polygon.clone());
                for r in &i.roads {
                    holes.push(map.get_r(*r).get_thick_polygon(map));
                }
            }
            // The convex hull illuminates a bit more of the surrounding area, looks better
            Polygon::with_holes(
                map.get_boundary_polygon().clone().into_ring(),
                vec![Polygon::convex_hull(holes).into_ring()],
            )
        };

        let mut groups = Vec::new();
        for i in &members {
            groups.extend(DrawTurnGroup::for_i(*i, &app.primary.map));
        }

        BundleEdits::synchronize(app, &members).apply(app);

        Box::new(NewTrafficSignalEditor {
            side_panel: make_side_panel(ctx, app, &members, 0),
            top_panel: make_top_panel(ctx, app, false, false),
            gameplay,
            members,
            current_phase: 0,
            groups,
            group_selected: None,
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
            fade_irrelevant: GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)]).upload(ctx),
        })
    }

    fn change_phase(&mut self, ctx: &mut EventCtx, app: &App, idx: usize) {
        if self.current_phase == idx {
            let mut new = make_side_panel(ctx, app, &self.members, self.current_phase);
            new.restore(ctx, &self.side_panel);
            self.side_panel = new;
        } else {
            self.current_phase = idx;
            self.side_panel = make_side_panel(ctx, app, &self.members, self.current_phase);
            // TODO Maybe center of previous member
            self.side_panel
                .scroll_to_member(ctx, format!("phase {}", idx + 1));
        }
    }
}

impl State for NewTrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        let canonical_signal = app
            .primary
            .map
            .get_traffic_signal(*self.members.iter().next().unwrap());
        let num_phases = canonical_signal.phases.len();

        match self.side_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Add new phase" {
                    let mut bundle = BundleEdits::get_current(app, &self.members);
                    self.command_stack.push(bundle.clone());
                    self.redo_stack.clear();
                    for ts in &mut bundle.signals {
                        ts.phases.push(Phase::new());
                    }
                    bundle.apply(app);

                    self.top_panel = make_top_panel(ctx, app, true, false);
                    self.change_phase(ctx, app, num_phases);
                    self.side_panel = make_side_panel(ctx, app, &self.members, self.current_phase);
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("change duration of phase ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    return Transition::Push(ChangeDuration::new(
                        ctx,
                        canonical_signal.phases[idx].phase_type.clone(),
                        idx,
                    ));
                }
                if let Some(x) = x.strip_prefix("delete phase ") {
                    let idx = x.parse::<usize>().unwrap() - 1;

                    let mut bundle = BundleEdits::get_current(app, &self.members);
                    self.command_stack.push(bundle.clone());
                    self.redo_stack.clear();
                    for ts in &mut bundle.signals {
                        ts.phases.remove(idx);
                    }
                    bundle.apply(app);

                    self.top_panel = make_top_panel(ctx, app, true, false);
                    // Don't use change_phase; it tries to preserve scroll
                    self.current_phase = if idx == num_phases - 1 { idx - 1 } else { idx };
                    self.side_panel = make_side_panel(ctx, app, &self.members, self.current_phase);
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("move up phase ") {
                    let idx = x.parse::<usize>().unwrap() - 1;

                    let mut bundle = BundleEdits::get_current(app, &self.members);
                    self.command_stack.push(bundle.clone());
                    self.redo_stack.clear();
                    for ts in &mut bundle.signals {
                        ts.phases.swap(idx, idx - 1);
                    }
                    bundle.apply(app);

                    self.top_panel = make_top_panel(ctx, app, true, false);
                    self.change_phase(ctx, app, idx - 1);
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("move down phase ") {
                    let idx = x.parse::<usize>().unwrap() - 1;

                    let mut bundle = BundleEdits::get_current(app, &self.members);
                    self.command_stack.push(bundle.clone());
                    self.redo_stack.clear();
                    for ts in &mut bundle.signals {
                        ts.phases.swap(idx, idx + 1);
                    }
                    bundle.apply(app);

                    self.top_panel = make_top_panel(ctx, app, true, false);
                    self.change_phase(ctx, app, idx + 1);
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("phase ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    self.change_phase(ctx, app, idx);
                    return Transition::Keep;
                }
                unreachable!()
            }
            _ => {}
        }

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Finish" => {
                    // TODO check_for_missing_groups
                    return Transition::Pop;
                }
                "Export" => {
                    for signal in BundleEdits::get_current(app, &self.members).signals {
                        let ts = signal.export(&app.primary.map);
                        abstutil::write_json(
                            format!("traffic_signal_data/{}.json", ts.intersection_osm_node_id),
                            &ts,
                        );
                    }
                }
                "Preview" => {
                    // Might have to do this first!
                    app.primary
                        .map
                        .recalculate_pathfinding_after_edits(&mut Timer::throwaway());

                    return Transition::Push(make_previewer(
                        ctx,
                        app,
                        self.members.clone(),
                        self.current_phase,
                    ));
                }
                "undo" => {
                    self.redo_stack
                        .push(BundleEdits::get_current(app, &self.members));
                    self.command_stack.pop().unwrap().apply(app);
                    self.top_panel = make_top_panel(ctx, app, !self.command_stack.is_empty(), true);
                    self.change_phase(ctx, app, 0);
                    return Transition::Keep;
                }
                "redo" => {
                    self.command_stack
                        .push(BundleEdits::get_current(app, &self.members));
                    self.redo_stack.pop().unwrap().apply(app);
                    self.top_panel = make_top_panel(ctx, app, true, !self.redo_stack.is_empty());
                    self.change_phase(ctx, app, 0);
                    return Transition::Keep;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        {
            if self.current_phase != 0 && ctx.input.key_pressed(Key::UpArrow) {
                self.change_phase(ctx, app, self.current_phase - 1);
            }

            if self.current_phase != num_phases - 1 && ctx.input.key_pressed(Key::DownArrow) {
                self.change_phase(ctx, app, self.current_phase + 1);
            }
        }

        if ctx.redo_mouseover() {
            self.group_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for g in &self.groups {
                    let signal = app.primary.map.get_traffic_signal(g.id.parent);
                    if g.block.contains_pt(pt) {
                        let phase = &signal.phases[self.current_phase];
                        let next_priority = match phase.get_priority_of_group(g.id) {
                            TurnPriority::Banned => {
                                if phase.could_be_protected(g.id, &signal.turn_groups) {
                                    Some(TurnPriority::Protected)
                                } else if g.id.crosswalk {
                                    None
                                } else {
                                    Some(TurnPriority::Yield)
                                }
                            }
                            TurnPriority::Yield => Some(TurnPriority::Banned),
                            TurnPriority::Protected => {
                                if g.id.crosswalk {
                                    Some(TurnPriority::Banned)
                                } else {
                                    Some(TurnPriority::Yield)
                                }
                            }
                        };
                        self.group_selected = Some((g.id, next_priority));
                        break;
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details
                .extend(self.members.clone());
            app.draw(g, opts, &app.primary.sim, &ShowEverything::new());
        }
        g.redraw(&self.fade_irrelevant);

        let mut batch = GeomBatch::new();
        for g in &self.groups {
            let signal = app.primary.map.get_traffic_signal(g.id.parent);
            if self
                .group_selected
                .as_ref()
                .map(|(id, _)| *id == g.id)
                .unwrap_or(false)
            {
                draw_selected_group(
                    app,
                    &mut batch,
                    g,
                    &signal.turn_groups[&g.id],
                    self.group_selected.unwrap().1,
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, g.block.clone());
                let phase = &signal.phases[self.current_phase];
                let arrow_color = match phase.get_priority_of_group(g.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn,
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, g.arrow.clone());
            }
        }
        batch.draw(g);

        self.top_panel.draw(g);
        self.side_panel.draw(g);
    }
}

fn make_side_panel(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    selected: usize,
) -> Composite {
    let map = &app.primary.map;

    let mut col = Vec::new();

    // Use any member for phase duration
    let canonical_signal = map.get_traffic_signal(*members.iter().next().unwrap());
    for (idx, canonical_phase) in canonical_signal.phases.iter().enumerate() {
        // Separator
        col.push(
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    // TODO draw_batch will scale up, but that's inappropriate here, since we're
                    // depending on window width, which already factors in scale
                    Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
                )]),
            )
            .centered_horiz(),
        );

        let unselected_btn = draw_multiple_signals(ctx, app, members, idx);
        let mut selected_btn = unselected_btn.clone();
        let bbox = unselected_btn.get_bounds().get_rectangle();
        selected_btn.push(Color::RED, bbox.to_outline(Distance::meters(5.0)).unwrap());
        let phase_btn = Btn::custom(unselected_btn, selected_btn, bbox).build(
            ctx,
            format!("phase {}", idx + 1),
            None,
        );

        let phase_col = Widget::col(vec![
            Widget::row(vec![
                match canonical_phase.phase_type {
                    PhaseType::Fixed(d) => Line(format!("Phase {}: {}", idx + 1, d)),
                    PhaseType::Adaptive(d) => Line(format!("Phase {}: {} (adaptive)", idx + 1, d)),
                }
                .small_heading()
                .draw(ctx),
                Btn::svg_def("system/assets/tools/edit.svg").build(
                    ctx,
                    format!("change duration of phase {}", idx + 1),
                    if selected == idx {
                        hotkey(Key::X)
                    } else {
                        None
                    },
                ),
                if canonical_signal.phases.len() > 1 {
                    Btn::svg_def("system/assets/tools/delete.svg")
                        .build(ctx, format!("delete phase {}", idx + 1), None)
                        .align_right()
                } else {
                    Widget::nothing()
                },
            ]),
            Widget::row(vec![
                phase_btn,
                Widget::col(vec![
                    if idx == 0 {
                        Btn::text_fg("↑").inactive(ctx)
                    } else {
                        Btn::text_fg("↑").build(ctx, format!("move up phase {}", idx + 1), None)
                    },
                    if idx == canonical_signal.phases.len() - 1 {
                        Btn::text_fg("↓").inactive(ctx)
                    } else {
                        Btn::text_fg("↓").build(ctx, format!("move down phase {}", idx + 1), None)
                    },
                ])
                .centered_vert()
                .align_right(),
            ]),
        ])
        .padding(10);

        if idx == selected {
            col.push(phase_col.bg(Color::hex("#2A2A2A")));
        } else {
            col.push(phase_col);
        }
    }

    // TODO Widget::separator(ctx, pct_width)
    col.push(
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(
                Color::WHITE,
                // TODO draw_batch will scale up, but that's inappropriate here, since we're
                // depending on window width, which already factors in scale
                Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
            )]),
        )
        .centered_horiz(),
    );
    col.push(Btn::text_fg("Add new phase").build_def(ctx, None));

    Composite::new(Widget::col(col))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .exact_size_percent(30, 85)
        .build(ctx)
}

fn draw_multiple_signals(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    idx: usize,
) -> GeomBatch {
    let mut batch = GeomBatch::new();
    for i in members {
        batch.push(
            app.cs.normal_intersection,
            app.primary.map.get_i(*i).polygon.clone(),
        );

        draw_signal_phase(
            ctx.prerender,
            &app.primary.map.get_traffic_signal(*i).phases[idx],
            *i,
            None,
            &mut batch,
            app,
            TrafficSignalStyle::Sidewalks,
        );
    }

    // Transform to a screen-space icon. How much should we scale things down?
    batch = batch.autocrop();
    let mut zoom: f64 = 1.0;
    if true {
        // Make the whole thing fit a fixed width
        let mut bounds = Bounds::new();
        for i in members {
            bounds.union(app.primary.map.get_i(*i).polygon.get_bounds());
        }
        zoom = 300.0 / bounds.width();
    } else {
        // Don't let any intersection get too small
        for i in members {
            zoom = zoom.max(150.0 / app.primary.map.get_i(*i).polygon.get_bounds().width());
        }
    }
    batch.scale(zoom)
}

impl BundleEdits {
    fn apply(&self, app: &mut App) {
        for s in &self.signals {
            app.primary.map.incremental_edit_traffic_signal(s.clone());
        }
    }

    fn get_current(app: &App, members: &BTreeSet<IntersectionID>) -> BundleEdits {
        let signals = members
            .iter()
            .map(|i| app.primary.map.get_traffic_signal(*i).clone())
            .collect();
        BundleEdits { signals }
    }

    // If the intersections haven't been edited together before, the number of phases and the
    // durations might not match up. Just initially force them to align somehow.
    fn synchronize(app: &App, members: &BTreeSet<IntersectionID>) -> BundleEdits {
        let map = &app.primary.map;
        // Pick one of the members with the most phases as canonical.
        let canonical = map.get_traffic_signal(
            *members
                .iter()
                .max_by_key(|i| map.get_traffic_signal(**i).phases.len())
                .unwrap(),
        );

        let mut signals = Vec::new();
        for i in members {
            let mut signal = map.get_traffic_signal(*i).clone();
            for (idx, canonical_phase) in canonical.phases.iter().enumerate() {
                if signal.phases.len() == idx {
                    signal.phases.push(Phase::new());
                }
                signal.phases[idx].phase_type = canonical_phase.phase_type.clone();
            }
            signals.push(signal);
        }

        BundleEdits { signals }
    }
}

struct ChangeDuration {
    composite: Composite,
    idx: usize,
}

impl ChangeDuration {
    fn new(ctx: &mut EventCtx, current: PhaseType, idx: usize) -> Box<dyn State> {
        Box::new(ChangeDuration {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("How long should this phase last?")
                        .small_heading()
                        .draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Seconds:".draw_text(ctx),
                    Spinner::new(
                        ctx,
                        (5, 300),
                        current.simple_duration().inner_seconds() as isize,
                    )
                    .named("duration"),
                ]),
                Widget::row(vec![
                    "Type:".draw_text(ctx),
                    Checkbox::toggle(
                        ctx,
                        "phase type",
                        "fixed",
                        "adaptive",
                        None,
                        match current {
                            PhaseType::Fixed(_) => true,
                            PhaseType::Adaptive(_) => false,
                        },
                    ),
                ]),
                Btn::text_bg2("Apply").build_def(ctx, hotkey(Key::Enter)),
            ]))
            .build(ctx),
            idx,
        })
    }
}

impl State for ChangeDuration {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "Apply" => {
                    let dt = Duration::seconds(self.composite.spinner("duration") as f64);
                    let new_type = if self.composite.is_checked("phase type") {
                        PhaseType::Fixed(dt)
                    } else {
                        PhaseType::Adaptive(dt)
                    };
                    let idx = self.idx;
                    return Transition::PopWithData(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<NewTrafficSignalEditor>().unwrap();

                        let mut bundle = BundleEdits::get_current(app, &editor.members);
                        editor.command_stack.push(bundle.clone());
                        editor.redo_stack.clear();
                        for ts in &mut bundle.signals {
                            ts.phases[idx].phase_type = new_type.clone();
                        }
                        bundle.apply(app);

                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        editor.change_phase(ctx, app, idx);
                        editor.side_panel =
                            make_side_panel(ctx, app, &editor.members, editor.current_phase);
                    }));
                }
                _ => unreachable!(),
            },
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}

// TODO I guess it's valid to preview without all turns possible. Some agents are just sad.
fn make_previewer(
    ctx: &mut EventCtx,
    app: &App,
    members: BTreeSet<IntersectionID>,
    phase: usize,
) -> Box<dyn State> {
    let random = "random agents around these intersections".to_string();
    let right_now = format!(
        "change the traffic signal live at {}",
        app.suspended_sim.as_ref().unwrap().time()
    );

    ChooseSomething::new(
        ctx,
        "Preview the traffic signal with what kind of traffic?",
        Choice::strings(vec![random, right_now]),
        Box::new(move |x, ctx, app| {
            if x == "random agents around these intersections" {
                for (idx, i) in members.iter().enumerate() {
                    if idx == 0 {
                        // Start at the current phase
                        let signal = app.primary.map.get_traffic_signal(*i);
                        // TODO Use the offset correctly
                        // TODO If there are adaptive phases, this could land anywhere
                        let mut step = Duration::ZERO;
                        for idx in 0..phase {
                            step += signal.phases[idx].phase_type.simple_duration();
                        }
                        app.primary.sim.timed_step(
                            &app.primary.map,
                            step,
                            &mut app.primary.sim_cb,
                            &mut Timer::throwaway(),
                        );
                    }

                    spawn_agents_around(*i, app);
                }
            } else {
                app.primary.sim = app.suspended_sim.as_ref().unwrap().clone();
                app.primary
                    .sim
                    .handle_live_edited_traffic_signals(&app.primary.map);
            }
            Transition::Replace(Box::new(PreviewTrafficSignal::new(ctx, app)))
        }),
    )
}
