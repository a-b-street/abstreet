mod edits;
mod picker;
mod preview;

use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{DrawBaselayer, PopupMsg, State, Transition};
use crate::options::TrafficSignalStyle;
use crate::render::{draw_signal_stage, DrawMovement, DrawOptions, BIG_ARROW_THICKNESS};
use crate::sandbox::GameplayMode;
use abstutil::Timer;
use geom::{ArrowCap, Distance, Duration, Line, Polygon, Pt2D};
use map_model::{
    ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Movement, MovementID,
    PhaseType, Stage, TurnPriority,
};
use std::collections::{BTreeSet, HashMap, VecDeque};
use widgetry::{
    hotkey, lctrl, Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, RewriteColor, Spinner, Text, TextExt, VerticalAlignment, Widget,
};

// Welcome to one of the most overwhelmingly complicated parts of the UI...

pub struct TrafficSignalEditor {
    side_panel: Panel,
    top_panel: Panel,

    mode: GameplayMode,
    members: BTreeSet<IntersectionID>,
    current_stage: usize,

    movements: Vec<DrawMovement>,
    // And the next priority to toggle to
    movement_selected: Option<(MovementID, Option<TurnPriority>)>,
    draw_current: Drawable,

    command_stack: Vec<BundleEdits>,
    redo_stack: Vec<BundleEdits>,
    // Before synchronizing the number of stages
    original: BundleEdits,
    warn_changed: bool,

    fade_irrelevant: Drawable,
}

// For every member intersection, the full state of that signal
#[derive(Clone, PartialEq)]
pub struct BundleEdits {
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

        let mut movements = Vec::new();
        for i in &members {
            movements.extend(DrawMovement::for_i(*i, &app.primary.map));
        }

        let original = BundleEdits::get_current(app, &members);
        let synced = BundleEdits::synchronize(app, &members);
        let warn_changed = original != synced;
        synced.apply(app);

        let mut editor = TrafficSignalEditor {
            side_panel: make_side_panel(ctx, app, &members, 0, None),
            top_panel: make_top_panel(ctx, app, false, false),
            mode,
            members,
            current_stage: 0,
            movements,
            movement_selected: None,
            draw_current: ctx.upload(GeomBatch::new()),
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
            warn_changed,
            original,
            fade_irrelevant: GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)]).upload(ctx),
        };
        editor.draw_current = editor.recalc_draw_current(ctx, app);
        Box::new(editor)
    }

    fn change_stage(&mut self, ctx: &mut EventCtx, app: &App, idx: usize) {
        let hovering = self.movement_selected.map(|(m, _)| m.parent);

        if self.current_stage == idx {
            let mut new = make_side_panel(ctx, app, &self.members, self.current_stage, hovering);
            new.restore(ctx, &self.side_panel);
            self.side_panel = new;
        } else {
            self.current_stage = idx;
            self.side_panel =
                make_side_panel(ctx, app, &self.members, self.current_stage, hovering);
            // TODO Maybe center of previous member
            self.side_panel
                .scroll_to_member(ctx, format!("stage {}", idx + 1));
        }

        self.draw_current = self.recalc_draw_current(ctx, app);
    }

    fn add_new_edit<F: Fn(&mut ControlTrafficSignal)>(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        idx: usize,
        fxn: F,
    ) {
        let mut bundle = BundleEdits::get_current(app, &self.members);
        self.command_stack.push(bundle.clone());
        self.redo_stack.clear();
        for ts in &mut bundle.signals {
            fxn(ts);
        }
        bundle.apply(app);

        self.top_panel = make_top_panel(ctx, app, true, false);
        self.change_stage(ctx, app, idx);
    }

    fn recalc_draw_current(&self, ctx: &mut EventCtx, app: &App) -> Drawable {
        let mut batch = GeomBatch::new();

        for i in &self.members {
            let signal = app.primary.map.get_traffic_signal(*i);
            let mut stage = signal.stages[self.current_stage].clone();
            if let Some((id, _)) = self.movement_selected {
                if id.parent == signal.id {
                    stage.edit_movement(&signal.movements[&id], TurnPriority::Banned);
                }
            }
            draw_signal_stage(
                ctx.prerender,
                &stage,
                signal.id,
                None,
                &mut batch,
                app,
                app.opts.traffic_signal_style.clone(),
            );
        }

        for m in &self.movements {
            let signal = app.primary.map.get_traffic_signal(m.id.parent);
            if self
                .movement_selected
                .as_ref()
                .map(|(id, _)| *id == m.id)
                .unwrap_or(false)
            {
                draw_selected_movement(
                    app,
                    &mut batch,
                    m,
                    &signal.movements[&m.id],
                    self.movement_selected.unwrap().1,
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, m.block.clone());
                let stage = &signal.stages[self.current_stage];
                let arrow_color = match stage.get_priority_of_movement(m.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn,
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, m.arrow.clone());
            }
        }
        ctx.upload(batch)
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if self.warn_changed {
            self.warn_changed = false;
            return Transition::Push(PopupMsg::new(
                ctx,
                "Note",
                vec!["Some signals were modified to match the number and duration of stages"],
            ));
        }

        ctx.canvas_movement();

        let canonical_signal = app
            .primary
            .map
            .get_traffic_signal(*self.members.iter().next().unwrap());
        let num_stages = canonical_signal.stages.len();

        match self.side_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Edit multiple signals" {
                    // First commit the current changes, so we enter SignalPicker with clean state.
                    // This UX flow is a little unintuitive.
                    let changes = check_for_missing_turns(app, &self.members)
                        .unwrap_or_else(|| BundleEdits::get_current(app, &self.members));
                    self.original.apply(app);
                    changes.commit(ctx, app);
                    return Transition::Replace(picker::SignalPicker::new(
                        ctx,
                        self.members.clone(),
                        self.mode.clone(),
                    ));
                }
                if x == "Edit entire signal" {
                    return Transition::Push(edits::edit_entire_signal(
                        ctx,
                        app,
                        canonical_signal.id,
                        self.mode.clone(),
                        self.original.clone(),
                    ));
                }
                if x == "Add new stage" {
                    self.add_new_edit(ctx, app, num_stages, |ts| {
                        ts.stages.push(Stage::new());
                    });
                    return Transition::Keep;
                }
                if x == "Apply offset" {
                    let offset = Duration::seconds(self.side_panel.spinner("offset") as f64);
                    self.add_new_edit(ctx, app, self.current_stage, |ts| {
                        ts.offset = offset;
                    });
                    return Transition::Keep;
                }
                if x == "Apply offsets" {
                    let offsets: HashMap<IntersectionID, Duration> = self
                        .members
                        .iter()
                        .enumerate()
                        .map(|(idx, i)| {
                            (
                                *i,
                                Duration::seconds(
                                    self.side_panel.spinner(&format!("offset {}", idx)) as f64,
                                ),
                            )
                        })
                        .collect();
                    self.add_new_edit(ctx, app, self.current_stage, |ts| {
                        ts.offset = offsets[&ts.id];
                    });
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("change duration of stage ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    return Transition::Push(edits::ChangeDuration::new(
                        ctx,
                        canonical_signal.stages[idx].phase_type.clone(),
                        idx,
                    ));
                }
                if let Some(x) = x.strip_prefix("delete stage ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    self.add_new_edit(ctx, app, 0, |ts| {
                        ts.stages.remove(idx);
                    });
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("move up stage ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    self.add_new_edit(ctx, app, idx - 1, |ts| {
                        ts.stages.swap(idx, idx - 1);
                    });
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("move down stage ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    self.add_new_edit(ctx, app, idx + 1, |ts| {
                        ts.stages.swap(idx, idx + 1);
                    });
                    return Transition::Keep;
                }
                if let Some(x) = x.strip_prefix("stage ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    self.change_stage(ctx, app, idx);
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

                        self.top_panel = make_top_panel(ctx, app, true, false);
                        self.change_stage(ctx, app, 0);

                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "Error: missing turns",
                            vec![
                                "Some turns are missing from this traffic signal",
                                "They've all been added as a new first stage. Please update your \
                                 changes to include them.",
                            ],
                        ));
                    } else {
                        let changes = BundleEdits::get_current(app, &self.members);
                        self.original.apply(app);
                        changes.commit(ctx, app);
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

                    return Transition::Push(preview::make_previewer(
                        ctx,
                        app,
                        self.members.clone(),
                        self.current_stage,
                    ));
                }
                "undo" => {
                    self.redo_stack
                        .push(BundleEdits::get_current(app, &self.members));
                    self.command_stack.pop().unwrap().apply(app);
                    self.top_panel = make_top_panel(ctx, app, !self.command_stack.is_empty(), true);
                    self.change_stage(ctx, app, 0);
                    return Transition::Keep;
                }
                "redo" => {
                    self.command_stack
                        .push(BundleEdits::get_current(app, &self.members));
                    self.redo_stack.pop().unwrap().apply(app);
                    self.top_panel = make_top_panel(ctx, app, true, !self.redo_stack.is_empty());
                    self.change_stage(ctx, app, 0);
                    return Transition::Keep;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        {
            if self.current_stage != 0 && ctx.input.key_pressed(Key::UpArrow) {
                self.change_stage(ctx, app, self.current_stage - 1);
            }

            if self.current_stage != num_stages - 1 && ctx.input.key_pressed(Key::DownArrow) {
                self.change_stage(ctx, app, self.current_stage + 1);
            }
        }

        if ctx.redo_mouseover() {
            let old = self.movement_selected.clone();

            self.movement_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for m in &self.movements {
                    let signal = app.primary.map.get_traffic_signal(m.id.parent);
                    if m.block.contains_pt(pt) {
                        let stage = &signal.stages[self.current_stage];
                        let next_priority = match stage.get_priority_of_movement(m.id) {
                            TurnPriority::Banned => {
                                if stage.could_be_protected(m.id, &signal.movements) {
                                    Some(TurnPriority::Protected)
                                } else if m.id.crosswalk {
                                    None
                                } else {
                                    Some(TurnPriority::Yield)
                                }
                            }
                            TurnPriority::Yield => Some(TurnPriority::Banned),
                            TurnPriority::Protected => {
                                if m.id.crosswalk {
                                    Some(TurnPriority::Banned)
                                } else {
                                    Some(TurnPriority::Yield)
                                }
                            }
                        };
                        self.movement_selected = Some((m.id, next_priority));
                        break;
                    }
                }
            }

            if self.movement_selected != old {
                self.draw_current = self.recalc_draw_current(ctx, app);
                self.change_stage(ctx, app, self.current_stage);
            }
        }

        if let Some((id, next_priority)) = self.movement_selected {
            if let Some(pri) = next_priority {
                let signal = app.primary.map.get_traffic_signal(id.parent);
                if app.per_obj.left_click(
                    ctx,
                    format!(
                        "toggle from {:?} to {:?}",
                        signal.stages[self.current_stage].get_priority_of_movement(id),
                        pri
                    ),
                ) {
                    let idx = self.current_stage;
                    let signal = signal.clone();
                    self.add_new_edit(ctx, app, idx, |ts| {
                        if ts.id == id.parent {
                            ts.stages[idx].edit_movement(&signal.movements[&id], pri);
                        }
                    });
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

        if let Some((id, _)) = self.movement_selected {
            let osd = if id.crosswalk {
                Text::from(Line(format!(
                    "Crosswalk across {}",
                    app.primary
                        .map
                        .get_r(id.from.id)
                        .get_name(app.opts.language.as_ref())
                )))
            } else {
                Text::from(Line(format!(
                    "Turn from {} to {}",
                    app.primary
                        .map
                        .get_r(id.from.id)
                        .get_name(app.opts.language.as_ref()),
                    app.primary
                        .map
                        .get_r(id.to.id)
                        .get_name(app.opts.language.as_ref())
                )))
            };
            CommonState::draw_custom_osd(g, app, osd);
        } else {
            CommonState::draw_osd(g, app);
        }
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App, can_undo: bool, can_redo: bool) -> Panel {
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
    Panel::new(Widget::col(vec![
        Line("Traffic signal editor").small_heading().draw(ctx),
        Widget::row(row),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_side_panel(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    selected: usize,
    hovering: Option<IntersectionID>,
) -> Panel {
    let map = &app.primary.map;
    // Use any member for stage duration
    let canonical_signal = map.get_traffic_signal(*members.iter().next().unwrap());

    let mut txt = Text::new();
    if members.len() == 1 {
        let i = *members.iter().next().unwrap();
        txt.add(Line(i.to_string()).big_heading_plain());

        let mut road_names = BTreeSet::new();
        for r in &app.primary.map.get_i(i).roads {
            road_names.insert(
                app.primary
                    .map
                    .get_r(*r)
                    .get_name(app.opts.language.as_ref()),
            );
        }
        for r in road_names {
            txt.add(Line(format!("  {}", r)));
        }
    } else {
        txt.add(Line(format!("{} intersections", members.len())).big_heading_plain());
    }
    {
        let mut total = Duration::ZERO;
        for s in &canonical_signal.stages {
            total += s.phase_type.simple_duration();
        }
        // TODO Say "normally" to account for adaptive stages?
        txt.add(Line(""));
        txt.add(Line(format!("One full cycle lasts {}", total)));
    }

    let mut col = vec![
        txt.draw(ctx),
        Btn::text_bg2("Edit multiple signals").build_def(ctx, hotkey(Key::M)),
    ];
    if members.len() == 1 {
        col.push(Btn::text_bg2("Edit entire signal").build_def(ctx, hotkey(Key::E)));
        col.push(Widget::row(vec![
            "Offset (s):".draw_text(ctx),
            Spinner::new(
                ctx,
                (0, 90),
                canonical_signal.offset.inner_seconds() as isize,
            )
            .named("offset"),
            Btn::text_bg2("Apply").build(ctx, "Apply offset", None),
        ]));
    }

    let translations = squish_polygons_together(
        members
            .iter()
            .map(|i| app.primary.map.get_i(*i).polygon.clone())
            .collect(),
    );

    for (idx, canonical_stage) in canonical_signal.stages.iter().enumerate() {
        col.push(Widget::horiz_separator(ctx, 0.2));

        let unselected_btn = draw_multiple_signals(ctx, app, members, idx, hovering, &translations);
        let mut selected_btn = unselected_btn.clone();
        let bbox = unselected_btn.get_bounds().get_rectangle();
        selected_btn.push(Color::RED, bbox.to_outline(Distance::meters(5.0)).unwrap());
        let stage_btn = Btn::custom(unselected_btn, selected_btn, bbox).build(
            ctx,
            format!("stage {}", idx + 1),
            None,
        );

        let stage_col = Widget::col(vec![
            Widget::row(vec![
                match canonical_stage.phase_type {
                    PhaseType::Fixed(d) => Line(format!("Stage {}: {}", idx + 1, d)),
                    PhaseType::Adaptive(d) => Line(format!("Stage {}: {} (adaptive)", idx + 1, d)),
                }
                .small_heading()
                .draw(ctx),
                Btn::svg_def("system/assets/tools/edit.svg").build(
                    ctx,
                    format!("change duration of stage {}", idx + 1),
                    if selected == idx {
                        hotkey(Key::X)
                    } else {
                        None
                    },
                ),
                if canonical_signal.stages.len() > 1 {
                    Btn::svg_def("system/assets/tools/delete.svg")
                        .build(ctx, format!("delete stage {}", idx + 1), None)
                        .align_right()
                } else {
                    Widget::nothing()
                },
            ]),
            Widget::row(vec![
                stage_btn,
                Widget::col(vec![
                    if idx == 0 {
                        Btn::text_fg("↑").inactive(ctx)
                    } else {
                        Btn::text_fg("↑").build(ctx, format!("move up stage {}", idx + 1), None)
                    },
                    if idx == canonical_signal.stages.len() - 1 {
                        Btn::text_fg("↓").inactive(ctx)
                    } else {
                        Btn::text_fg("↓").build(ctx, format!("move down stage {}", idx + 1), None)
                    },
                ])
                .centered_vert()
                .align_right(),
            ]),
        ])
        .padding(10);

        if idx == selected {
            col.push(stage_col.bg(Color::hex("#2A2A2A")));
        } else {
            col.push(stage_col);
        }
    }

    col.push(Widget::horiz_separator(ctx, 0.2));
    col.push(Btn::text_fg("Add new stage").build_def(ctx, None));

    // TODO This doesn't even have a way of knowing which spinner corresponds to which
    // intersection!
    if members.len() > 1 {
        col.push("Tune offset (s)".draw_text(ctx));
        col.push(
            Widget::row(
                members
                    .iter()
                    .enumerate()
                    .map(|(idx, i)| {
                        let spinner = Spinner::new(
                            ctx,
                            (0, 90),
                            map.get_traffic_signal(*i).offset.inner_seconds() as isize,
                        )
                        .named(format!("offset {}", idx));
                        if hovering == Some(*i) {
                            spinner.padding(2).outline(2.0, Color::YELLOW)
                        } else {
                            spinner
                        }
                    })
                    .collect(),
            )
            .evenly_spaced(),
        );
        col.push(Btn::text_bg2("Apply").build(ctx, "Apply offsets", None));
    }

    Panel::new(Widget::col(col))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .exact_size_percent(30, 85)
        .build(ctx)
}

impl BundleEdits {
    fn apply(&self, app: &mut App) {
        for s in &self.signals {
            app.primary.map.incremental_edit_traffic_signal(s.clone());
        }
    }

    fn commit(self, ctx: &mut EventCtx, app: &mut App) {
        // Skip if there's no change
        if self == BundleEdits::get_current(app, &self.signals.iter().map(|s| s.id).collect()) {
            return;
        }

        let mut edits = app.primary.map.get_edits().clone();
        // TODO Can we batch these commands somehow, so undo/redo in edit mode behaves properly?
        for signal in self.signals {
            edits.commands.push(EditCmd::ChangeIntersection {
                i: signal.id,
                old: app.primary.map.get_i_edit(signal.id),
                new: EditIntersection::TrafficSignal(signal.export(&app.primary.map)),
            });
        }
        apply_map_edits(ctx, app, edits);
    }

    fn get_current(app: &App, members: &BTreeSet<IntersectionID>) -> BundleEdits {
        let signals = members
            .iter()
            .map(|i| app.primary.map.get_traffic_signal(*i).clone())
            .collect();
        BundleEdits { signals }
    }

    // If the intersections haven't been edited together before, the number of stages and the
    // durations might not match up. Just initially force them to align somehow.
    fn synchronize(app: &App, members: &BTreeSet<IntersectionID>) -> BundleEdits {
        let map = &app.primary.map;
        // Pick one of the members with the most stages as canonical.
        let canonical = map.get_traffic_signal(
            *members
                .iter()
                .max_by_key(|i| map.get_traffic_signal(**i).stages.len())
                .unwrap(),
        );

        let mut signals = Vec::new();
        for i in members {
            let mut signal = map.get_traffic_signal(*i).clone();
            for (idx, canonical_stage) in canonical.stages.iter().enumerate() {
                if signal.stages.len() == idx {
                    signal.stages.push(Stage::new());
                }
                signal.stages[idx].phase_type = canonical_stage.phase_type.clone();
            }
            signals.push(signal);
        }

        BundleEdits { signals }
    }
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
    // Stick all the missing turns in a new stage at the beginning.
    for signal in &mut bundle.signals {
        let mut stage = Stage::new();
        // TODO Could do this more efficiently
        for m in &all_missing {
            if m.parent != signal.id {
                continue;
            }
            if m.crosswalk {
                stage.protected_movements.insert(*m);
            } else {
                stage.yield_movements.insert(*m);
            }
        }
        signal.stages.insert(0, stage);
    }
    Some(bundle)
}

fn draw_multiple_signals(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    idx: usize,
    hovering: Option<IntersectionID>,
    translations: &Vec<(f64, f64)>,
) -> GeomBatch {
    let mut batch = GeomBatch::new();
    for (i, (dx, dy)) in members.iter().zip(translations) {
        let mut piece = GeomBatch::new();
        piece.push(
            app.cs.normal_intersection,
            app.primary.map.get_i(*i).polygon.clone(),
        );
        draw_signal_stage(
            ctx.prerender,
            &app.primary.map.get_traffic_signal(*i).stages[idx],
            *i,
            None,
            &mut piece,
            app,
            TrafficSignalStyle::Sidewalks,
        );
        if members.len() > 1 && hovering.map(|x| x == *i).unwrap_or(false) {
            // TODO This makes the side-panel jump a little, because the outline slightly increases
            // the bounds...
            if let Ok(p) = app
                .primary
                .map
                .get_i(*i)
                .polygon
                .to_outline(Distance::meters(0.1))
            {
                piece.push(Color::YELLOW, p);
            }
        }
        batch.append(piece.translate(*dx, *dy));
    }

    // Make the whole thing fit a fixed width
    batch = batch.autocrop();
    let bounds = batch.get_bounds();
    let zoom = (300.0 / bounds.width()).min(300.0 / bounds.height());
    batch.scale(zoom)
}

fn draw_selected_movement(
    app: &App,
    batch: &mut GeomBatch,
    g: &DrawMovement,
    m: &Movement,
    next_priority: Option<TurnPriority>,
) {
    // TODO Refactor this mess. Maybe after things like "dashed with outline" can be expressed more
    // composably like SVG, using lyon.
    let block_color = match next_priority {
        Some(TurnPriority::Protected) => {
            let green = Color::hex("#72CE36");
            let arrow = m.geom.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
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
                m.geom.dashed_arrow(
                    BIG_ARROW_THICKNESS,
                    Distance::meters(1.2),
                    Distance::meters(0.3),
                    ArrowCap::Triangle,
                ),
            );
            batch.extend(
                app.cs.signal_permitted_turn.alpha(0.8),
                m.geom
                    .exact_slice(
                        Distance::meters(0.1),
                        m.geom.length() - Distance::meters(0.1),
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
            let arrow = m.geom.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
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

// TODO Move to geom?
fn squish_polygons_together(mut polygons: Vec<Polygon>) -> Vec<(f64, f64)> {
    if polygons.len() == 1 {
        return vec![(0.0, 0.0)];
    }

    // Can't be too big, or polygons could silently swap places. To be careful, pick something a
    // bit smaller than the smallest polygon.
    let step_size = 0.8
        * polygons.iter().fold(std::f64::MAX, |x, p| {
            x.min(p.get_bounds().width()).min(p.get_bounds().height())
        });

    let mut translations: Vec<(f64, f64)> =
        std::iter::repeat((0.0, 0.0)).take(polygons.len()).collect();
    // Once a polygon hits another while moving, stop adjusting it. Otherwise, go round-robin.
    let mut indices: VecDeque<usize> = (0..polygons.len()).collect();

    let mut attempts = 0;
    while !indices.is_empty() {
        let idx = indices.pop_front().unwrap();
        let center = Pt2D::center(&polygons.iter().map(|p| p.center()).collect());
        let angle = Line::must_new(polygons[idx].center(), center).angle();
        let pt = Pt2D::new(0.0, 0.0).project_away(Distance::meters(step_size), angle);

        // Do we hit anything if we move this way?
        let translated = polygons[idx].translate(pt.x(), pt.y());
        if polygons
            .iter()
            .enumerate()
            .any(|(i, p)| i != idx && !translated.intersection(p).is_empty())
        {
            // Stop moving this polygon
        } else {
            translations[idx].0 += pt.x();
            translations[idx].1 += pt.y();
            polygons[idx] = translated;
            indices.push_back(idx);
        }

        attempts += 1;
        if attempts == 100 {
            break;
        }
    }

    translations
}
