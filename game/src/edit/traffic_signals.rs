use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::edit::{apply_map_edits, check_sidewalk_connectivity, StopSignEditor};
use crate::game::{ChooseSomething, DrawBaselayer, PopupMsg, State, Transition};
use crate::options::TrafficSignalStyle;
use crate::render::{draw_signal_phase, DrawOptions, DrawTurnGroup, BIG_ARROW_THICKNESS};
use crate::sandbox::{spawn_agents_around, GameplayMode, SpeedControls, TimePanel};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Checkbox, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, RewriteColor, Spinner, Text, TextExt, UpdateType,
    VerticalAlignment, Widget,
};
use geom::{ArrowCap, Bounds, Distance, Duration, Polygon};
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Phase,
    PhaseType, TurnGroup, TurnGroupID, TurnPriority,
};
use std::collections::BTreeSet;

// Welcome to one of the most overwhelmingly complicated parts of the UI...

pub struct TrafficSignalEditor {
    side_panel: Composite,
    top_panel: Composite,

    mode: GameplayMode,
    members: BTreeSet<IntersectionID>,
    current_phase: usize,

    groups: Vec<DrawTurnGroup>,
    // And the next priority to toggle to
    group_selected: Option<(TurnGroupID, Option<TurnPriority>)>,
    draw_current: Drawable,

    command_stack: Vec<BundleEdits>,
    redo_stack: Vec<BundleEdits>,
    // Before synchronizing the number of phases
    original: BundleEdits,

    fade_irrelevant: Drawable,
}

// For every member intersection, the full state of that signal
#[derive(Clone)]
struct BundleEdits {
    signals: Vec<ControlTrafficSignal>,
}

impl TrafficSignalEditor {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        members: BTreeSet<IntersectionID>,
        mode: GameplayMode,
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

        let original = BundleEdits::get_current(app, &members);
        BundleEdits::synchronize(app, &members).apply(app);

        let mut editor = TrafficSignalEditor {
            side_panel: make_side_panel(ctx, app, &members, 0),
            top_panel: make_top_panel(ctx, app, false, false),
            mode,
            members,
            current_phase: 0,
            groups,
            group_selected: None,
            draw_current: ctx.upload(GeomBatch::new()),
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
            original,
            fade_irrelevant: GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)]).upload(ctx),
        };
        editor.draw_current = editor.recalc_draw_current(ctx, app);
        Box::new(editor)
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

        self.draw_current = self.recalc_draw_current(ctx, app);
    }

    fn recalc_draw_current(&self, ctx: &mut EventCtx, app: &App) -> Drawable {
        let mut batch = GeomBatch::new();

        for i in &self.members {
            let signal = app.primary.map.get_traffic_signal(*i);
            let mut phase = signal.phases[self.current_phase].clone();
            if let Some((id, _)) = self.group_selected {
                if id.parent == signal.id {
                    phase.edit_group(&signal.turn_groups[&id], TurnPriority::Banned);
                }
            }
            draw_signal_phase(
                ctx.prerender,
                &phase,
                signal.id,
                None,
                &mut batch,
                app,
                app.opts.traffic_signal_style.clone(),
            );
        }

        for tg in &self.groups {
            let signal = app.primary.map.get_traffic_signal(tg.id.parent);
            if self
                .group_selected
                .as_ref()
                .map(|(id, _)| *id == tg.id)
                .unwrap_or(false)
            {
                draw_selected_group(
                    app,
                    &mut batch,
                    tg,
                    &signal.turn_groups[&tg.id],
                    self.group_selected.unwrap().1,
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, tg.block.clone());
                let phase = &signal.phases[self.current_phase];
                let arrow_color = match phase.get_priority_of_group(tg.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn,
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, tg.arrow.clone());
            }
        }
        ctx.upload(batch)
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        let canonical_signal = app
            .primary
            .map
            .get_traffic_signal(*self.members.iter().next().unwrap());
        let num_phases = canonical_signal.phases.len();

        match self.side_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Edit entire signal" {
                    return Transition::Push(edit_entire_signal(
                        ctx,
                        app,
                        canonical_signal.id,
                        self.mode.clone(),
                        self.original.clone(),
                    ));
                }
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
                    if let Some(bundle) = check_for_missing_turns(app, &self.members) {
                        bundle.apply(app);
                        self.command_stack.push(bundle.clone());
                        self.redo_stack.clear();

                        self.current_phase = 0;
                        self.top_panel = make_top_panel(ctx, app, true, false);
                        self.change_phase(ctx, app, self.current_phase);

                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "Error: missing turns",
                            vec![
                                "Some turns are missing from this traffic signal",
                                "They've all been added as a new first phase. Please update your \
                                 changes to include them.",
                            ],
                        ));
                    } else {
                        let changes = BundleEdits::get_current(app, &self.members);
                        self.original.apply(app);

                        let mut edits = app.primary.map.get_edits().clone();
                        // TODO Can we batch these commands somehow, so undo/redo in edit mode
                        // behaves properly?
                        for signal in changes.signals {
                            edits.commands.push(EditCmd::ChangeIntersection {
                                i: signal.id,
                                old: app.primary.map.get_i_edit(signal.id),
                                new: EditIntersection::TrafficSignal(
                                    signal.export(&app.primary.map),
                                ),
                            });
                        }
                        apply_map_edits(ctx, app, edits);
                        return Transition::Pop;
                    }
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
            let old = self.group_selected.clone();

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

            if self.group_selected != old {
                self.draw_current = self.recalc_draw_current(ctx, app);
            }
        }

        if let Some((id, next_priority)) = self.group_selected {
            if let Some(pri) = next_priority {
                let signal = app.primary.map.get_traffic_signal(id.parent);
                if app.per_obj.left_click(
                    ctx,
                    format!(
                        "toggle from {:?} to {:?}",
                        signal.phases[self.current_phase].get_priority_of_group(id),
                        pri
                    ),
                ) {
                    let mut bundle = BundleEdits::get_current(app, &self.members);
                    self.command_stack.push(bundle.clone());
                    self.redo_stack.clear();
                    for ts in &mut bundle.signals {
                        if ts.id == id.parent {
                            ts.phases[self.current_phase].edit_group(&signal.turn_groups[&id], pri);
                            break;
                        }
                    }
                    bundle.apply(app);

                    self.top_panel = make_top_panel(ctx, app, true, false);
                    self.change_phase(ctx, app, self.current_phase);
                    return Transition::KeepWithMouseover;
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
        g.redraw(&self.draw_current);

        self.top_panel.draw(g);
        self.side_panel.draw(g);

        if let Some((id, _)) = self.group_selected {
            let osd = if id.crosswalk {
                Text::from(Line(format!(
                    "Crosswalk across {}",
                    app.primary.map.get_r(id.from.id).get_name()
                )))
            } else {
                Text::from(Line(format!(
                    "Turn from {} to {}",
                    app.primary.map.get_r(id.from.id).get_name(),
                    app.primary.map.get_r(id.to.id).get_name()
                )))
            };
            CommonState::draw_custom_osd(g, app, osd);
        } else {
            CommonState::draw_osd(g, app);
        }
    }
}

fn make_side_panel(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    selected: usize,
) -> Composite {
    let map = &app.primary.map;
    // Use any member for phase duration
    let canonical_signal = map.get_traffic_signal(*members.iter().next().unwrap());

    let mut txt = Text::new();
    if members.len() == 1 {
        let i = *members.iter().next().unwrap();
        txt.add(Line(i.to_string()).big_heading_plain());

        let mut road_names = BTreeSet::new();
        for r in &app.primary.map.get_i(i).roads {
            road_names.insert(app.primary.map.get_r(*r).get_name());
        }
        for r in road_names {
            txt.add(Line(format!("- {}", r)));
        }
    } else {
        txt.add(Line(format!("{} intersections", members.len())).big_heading_plain());
    }
    {
        let mut total = Duration::ZERO;
        for p in &canonical_signal.phases {
            total += p.phase_type.simple_duration();
        }
        // TODO Say "normally" to account for adaptive phases?
        txt.add(Line(""));
        txt.add(Line(format!("One full cycle lasts {}", total)));
    }

    let mut col = vec![txt.draw(ctx)];
    if members.len() == 1 {
        col.push(Btn::text_bg2("Edit entire signal").build_def(ctx, hotkey(Key::E)));
    }

    for (idx, canonical_phase) in canonical_signal.phases.iter().enumerate() {
        col.push(Widget::horiz_separator(ctx, 0.2));

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

    col.push(Widget::horiz_separator(ctx, 0.2));
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
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                        let mut bundle = BundleEdits::get_current(app, &editor.members);
                        editor.command_stack.push(bundle.clone());
                        editor.redo_stack.clear();
                        for ts in &mut bundle.signals {
                            ts.phases[idx].phase_type = new_type.clone();
                        }
                        bundle.apply(app);

                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        editor.change_phase(ctx, app, idx);
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

// If None, nothing missing.
fn check_for_missing_turns(app: &App, members: &BTreeSet<IntersectionID>) -> Option<BundleEdits> {
    let mut all_missing = BTreeSet::new();
    for i in members {
        all_missing.extend(app.primary.map.get_traffic_signal(*i).missing_turns());
    }
    if all_missing.is_empty() {
        return None;
    }

    let mut bundle = BundleEdits::get_current(app, members);
    // Stick all the missing turns in a new phase at the beginning.
    for signal in &mut bundle.signals {
        let mut phase = Phase::new();
        // TODO Could do this more efficiently
        for g in &all_missing {
            if g.parent != signal.id {
                continue;
            }
            if g.crosswalk {
                phase.protected_groups.insert(*g);
            } else {
                phase.yield_groups.insert(*g);
            }
        }
        signal.phases.insert(0, phase);
    }
    Some(bundle)
}

fn edit_entire_signal(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    mode: GameplayMode,
    original: BundleEdits,
) -> Box<dyn State> {
    let has_sidewalks = app
        .primary
        .map
        .get_turns_in_intersection(i)
        .any(|t| t.between_sidewalks());

    let use_template = "use template";
    let all_walk = "add an all-walk phase at the end";
    let stop_sign = "convert to stop signs";
    let close = "close intersection for construction";
    let reset = "reset to default";

    let mut choices = vec![use_template];
    if has_sidewalks {
        choices.push(all_walk);
    }
    // TODO Conflating stop signs and construction here
    if mode.can_edit_stop_signs() {
        choices.push(stop_sign);
        choices.push(close);
    }
    choices.push(reset);

    ChooseSomething::new(
        ctx,
        "What do you want to change?",
        Choice::strings(choices),
        Box::new(move |x, ctx, app| match x.as_str() {
            x if x == use_template => Transition::Replace(ChooseSomething::new(
                ctx,
                "Use which preset for this intersection?",
                Choice::from(ControlTrafficSignal::get_possible_policies(
                    &app.primary.map,
                    i,
                    &mut Timer::throwaway(),
                )),
                Box::new(move |new_signal, _, _| {
                    Transition::PopWithData(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                        let mut bundle = BundleEdits::get_current(app, &editor.members);
                        editor.command_stack.push(bundle.clone());
                        editor.redo_stack.clear();
                        bundle.signals = vec![new_signal];
                        bundle.apply(app);

                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        editor.change_phase(ctx, app, 0);
                    }))
                }),
            )),
            x if x == all_walk => Transition::PopWithData(Box::new(move |state, ctx, app| {
                let mut new_signal = app.primary.map.get_traffic_signal(i).clone();
                if new_signal.convert_to_ped_scramble() {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                    let mut bundle = BundleEdits::get_current(app, &editor.members);
                    editor.command_stack.push(bundle.clone());
                    editor.redo_stack.clear();
                    bundle.signals = vec![new_signal];
                    bundle.apply(app);

                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    editor.change_phase(ctx, app, 0);
                }
            })),
            x if x == stop_sign => {
                original.apply(app);

                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i,
                    old: app.primary.map.get_i_edit(i),
                    new: EditIntersection::StopSign(ControlStopSign::new(&app.primary.map, i)),
                });
                apply_map_edits(ctx, app, edits);
                Transition::PopThenReplace(Box::new(StopSignEditor::new(ctx, app, i, mode.clone())))
            }
            x if x == close => {
                original.apply(app);

                let cmd = EditCmd::ChangeIntersection {
                    i,
                    old: app.primary.map.get_i_edit(i),
                    new: EditIntersection::Closed,
                };
                if let Some(err) = check_sidewalk_connectivity(ctx, app, cmd.clone()) {
                    Transition::Replace(err)
                } else {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(cmd);
                    apply_map_edits(ctx, app, edits);

                    Transition::PopTwice
                }
            }
            x if x == reset => Transition::PopWithData(Box::new(move |state, ctx, app| {
                let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                let mut bundle = BundleEdits::get_current(app, &editor.members);
                editor.command_stack.push(bundle.clone());
                editor.redo_stack.clear();
                let new_signal = ControlTrafficSignal::get_possible_policies(
                    &app.primary.map,
                    i,
                    &mut Timer::throwaway(),
                )
                .remove(0)
                .1;
                bundle.signals = vec![new_signal];
                bundle.apply(app);

                editor.top_panel = make_top_panel(ctx, app, true, false);
                editor.change_phase(ctx, app, 0);
            })),
            _ => unreachable!(),
        }),
    )
}

fn make_top_panel(ctx: &mut EventCtx, app: &App, can_undo: bool, can_redo: bool) -> Composite {
    let row = vec![
        Btn::text_fg("Finish").build_def(ctx, hotkey(Key::Escape)),
        Btn::text_fg("Preview").build_def(ctx, lctrl(Key::P)),
        (if can_undo {
            Btn::svg_def("system/assets/tools/undo.svg").build(ctx, "undo", lctrl(Key::Z))
        } else {
            Widget::draw_svg_transform(
                ctx,
                "system/assets/tools/undo.svg",
                RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
            )
        })
        .centered_vert(),
        (if can_redo {
            Btn::svg_def("system/assets/tools/redo.svg").build(
                ctx,
                "redo",
                // TODO ctrl+shift+Z!
                lctrl(Key::Y),
            )
        } else {
            Widget::draw_svg_transform(
                ctx,
                "system/assets/tools/redo.svg",
                RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
            )
        })
        .centered_vert(),
        if app.opts.dev {
            Btn::text_fg("Export")
                .tooltip(Text::from_multiline(vec![
                    Line("This will create a JSON file in traffic_signal_data/.").small(),
                    Line(
                        "Contribute this to map how this traffic signal is currently timed in \
                         real life.",
                    )
                    .small(),
                ]))
                .build_def(ctx, None)
        } else {
            Widget::nothing()
        },
    ];
    Composite::new(Widget::col(vec![
        Line("Traffic signal editor").small_heading().draw(ctx),
        Widget::row(row),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

// TODO Show diagram, auto-sync the phase.
// TODO Auto quit after things are gone?
struct PreviewTrafficSignal {
    composite: Composite,
    speed: SpeedControls,
    time_panel: TimePanel,
}

impl PreviewTrafficSignal {
    fn new(ctx: &mut EventCtx, app: &App) -> PreviewTrafficSignal {
        PreviewTrafficSignal {
            composite: Composite::new(Widget::col(vec![
                "Previewing traffic signal".draw_text(ctx),
                Btn::text_fg("back to editing").build_def(ctx, hotkey(Key::Escape)),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            speed: SpeedControls::new(ctx, app),
            time_panel: TimePanel::new(ctx, app),
        }
    }
}

impl State for PreviewTrafficSignal {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back to editing" => {
                    app.primary.clear_sim();
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        self.time_panel.event(ctx, app);
        // TODO Ideally here reset to midnight would jump back to when the preview started?
        if let Some(t) = self.speed.event(ctx, app, None) {
            return t;
        }
        if self.speed.is_paused() {
            Transition::Keep
        } else {
            ctx.request_update(UpdateType::Game);
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
        self.speed.draw(g);
        self.time_panel.draw(g);
    }
}

fn draw_selected_group(
    app: &App,
    batch: &mut GeomBatch,
    g: &DrawTurnGroup,
    tg: &TurnGroup,
    next_priority: Option<TurnPriority>,
) {
    // TODO Refactor this mess. Maybe after things like "dashed with outline" can be expressed more
    // composably like SVG, using lyon.
    let block_color = match next_priority {
        Some(TurnPriority::Protected) => {
            let green = Color::hex("#72CE36");
            let arrow = tg.geom.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
            batch.push(green.alpha(0.5), arrow.clone());
            if let Ok(p) = arrow.to_outline(Distance::meters(0.1)) {
                batch.push(green, p);
            }
            green
        }
        Some(TurnPriority::Yield) => {
            batch.extend(
                // TODO Ideally the inner part would be the lower opacity blue, but can't yet
                // express that it should cover up the thicker solid blue beneath it
                Color::BLACK.alpha(0.8),
                tg.geom.dashed_arrow(
                    BIG_ARROW_THICKNESS,
                    Distance::meters(1.2),
                    Distance::meters(0.3),
                    ArrowCap::Triangle,
                ),
            );
            batch.extend(
                app.cs.signal_permitted_turn.alpha(0.8),
                tg.geom
                    .exact_slice(
                        Distance::meters(0.1),
                        tg.geom.length() - Distance::meters(0.1),
                    )
                    .dashed_arrow(
                        BIG_ARROW_THICKNESS / 2.0,
                        Distance::meters(1.0),
                        Distance::meters(0.5),
                        ArrowCap::Triangle,
                    ),
            );
            app.cs.signal_permitted_turn
        }
        Some(TurnPriority::Banned) => {
            let red = Color::hex("#EB3223");
            let arrow = tg.geom.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
            batch.push(red.alpha(0.5), arrow.clone());
            if let Ok(p) = arrow.to_outline(Distance::meters(0.1)) {
                batch.push(red, p);
            }
            red
        }
        None => app.cs.signal_turn_block_bg,
    };
    batch.push(block_color, g.block.clone());
    batch.push(Color::WHITE, g.arrow.clone());
}
