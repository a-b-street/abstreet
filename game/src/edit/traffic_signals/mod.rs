use std::collections::{BTreeSet, VecDeque};

use abstutil::Timer;
use geom::{Distance, Line, Polygon, Pt2D};
use map_gui::options::TrafficSignalStyle;
use map_gui::render::{traffic_signal, DrawMovement, DrawOptions};
use map_gui::tools::PopupMsg;
use map_model::{
    ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, MovementID, Stage, StageType,
    TurnPriority,
};
use widgetry::{
    include_labeled_bytes, lctrl, Color, ControlState, DrawBaselayer, Drawable, EventCtx,
    GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, MultiButton, Outcome, Panel, RewriteColor,
    State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, ShowEverything, Transition};
use crate::common::{CommonState, Warping};
use crate::edit::{apply_map_edits, ConfirmDiscard};
use crate::sandbox::GameplayMode;

mod edits;
mod gmns;
mod offsets;
mod picker;
mod preview;

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
    tooltip: Option<Text>,

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
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        members: BTreeSet<IntersectionID>,
        mode: GameplayMode,
    ) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let original = BundleEdits::get_current(app, &members);
        let synced = BundleEdits::synchronize(app, &members);
        let warn_changed = original != synced;
        synced.apply(app);

        let mut editor = TrafficSignalEditor {
            side_panel: make_side_panel(ctx, app, &members, 0),
            top_panel: make_top_panel(ctx, app, false, false),
            mode,
            current_stage: 0,
            movements: Vec::new(),
            movement_selected: None,
            draw_current: Drawable::empty(ctx),
            tooltip: None,
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
            warn_changed,
            original,
            fade_irrelevant: fade_irrelevant(app, &members).upload(ctx),
            members,
        };
        editor.recalc_draw_current(ctx, app);
        Box::new(editor)
    }

    fn change_stage(&mut self, ctx: &mut EventCtx, app: &App, idx: usize) {
        if self.current_stage == idx {
            let mut new = make_side_panel(ctx, app, &self.members, self.current_stage);
            new.restore(ctx, &self.side_panel);
            self.side_panel = new;
        } else {
            self.current_stage = idx;
            self.side_panel = make_side_panel(ctx, app, &self.members, self.current_stage);
        }

        self.recalc_draw_current(ctx, app);
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

    fn recalc_draw_current(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        let mut movements = Vec::new();
        for i in &self.members {
            let stage = &app.primary.map.get_traffic_signal(*i).stages[self.current_stage];
            for (m, draw) in DrawMovement::for_i(
                ctx.prerender,
                &app.primary.map,
                &app.cs,
                *i,
                self.current_stage,
            ) {
                if self
                    .movement_selected
                    .map(|(x, _)| x != m.id)
                    .unwrap_or(true)
                    || m.id.crosswalk
                {
                    batch.append(draw);
                } else if !stage.protected_movements.contains(&m.id)
                    && !stage.yield_movements.contains(&m.id)
                {
                    // Still draw the icon, but highlight it
                    batch.append(draw.color(RewriteColor::Change(
                        Color::hex("#7C7C7C"),
                        Color::hex("#72CE36"),
                    )));
                }
                movements.push(m);
            }
            traffic_signal::draw_stage_number(
                app,
                ctx.prerender,
                *i,
                self.current_stage,
                &mut batch,
            );
        }

        // Draw the selected thing on top of everything else
        if let Some((selected, next_priority)) = self.movement_selected {
            for m in &movements {
                if m.id == selected {
                    m.draw_selected_movement(app, &mut batch, next_priority);
                    break;
                }
            }
        }

        self.draw_current = ctx.upload(batch);
        self.movements = movements;
    }
}

impl State<App> for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.tooltip = None;

        if self.warn_changed {
            self.warn_changed = false;
            return Transition::Push(PopupMsg::new_state(
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

        if let Outcome::Clicked(x) = self.side_panel.event(ctx) {
            match x.as_ref() {
                "Edit entire signal" => {
                    return Transition::Push(edits::edit_entire_signal(
                        ctx,
                        app,
                        canonical_signal.id,
                        self.mode.clone(),
                        self.original.clone(),
                    ));
                }
                "Tune offsets between signals" => {
                    return Transition::Push(offsets::ShowAbsolute::new_state(
                        ctx,
                        app,
                        self.members.clone(),
                    ));
                }
                "Add a new stage" => {
                    self.add_new_edit(ctx, app, num_stages, |ts| {
                        ts.stages.push(Stage::new());
                    });
                    return Transition::Keep;
                }
                "change duration" => {
                    return Transition::Push(edits::ChangeDuration::new_state(
                        ctx,
                        app,
                        &canonical_signal,
                        self.current_stage,
                    ));
                }
                "delete stage" => {
                    let idx = self.current_stage;
                    self.add_new_edit(ctx, app, 0, |ts| {
                        ts.stages.remove(idx);
                    });
                    return Transition::Keep;
                }
                "move stage left" => {
                    let idx = self.current_stage;
                    self.add_new_edit(ctx, app, idx - 1, |ts| {
                        ts.stages.swap(idx, idx - 1);
                    });
                    return Transition::Keep;
                }
                "move stage right" => {
                    let idx = self.current_stage;
                    self.add_new_edit(ctx, app, idx + 1, |ts| {
                        ts.stages.swap(idx, idx + 1);
                    });
                    return Transition::Keep;
                }
                x => {
                    if let Some(x) = x.strip_prefix("stage ") {
                        // 123, Intersection #456
                        let parts = x.split(", Intersection #").collect::<Vec<_>>();
                        let idx = parts[0].parse::<usize>().unwrap() - 1;
                        let i = IntersectionID(parts[1].parse::<usize>().unwrap());
                        self.change_stage(ctx, app, idx);
                        let center = app.primary.map.get_i(i).polygon.center();
                        // Constantly warping is really annoying, only do it if the intersection is
                        // offscreen
                        if ctx.canvas.get_screen_bounds().contains(center) {
                            return Transition::Keep;
                        } else {
                            return Transition::Push(Warping::new_state(
                                ctx,
                                center,
                                Some(15.0),
                                None,
                                &mut app.primary,
                            ));
                        }
                    } else {
                        unreachable!()
                    }
                }
            }
        }

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Finish" => {
                    if let Some(bundle) = check_for_missing_turns(app, &self.members) {
                        bundle.apply(app);
                        self.command_stack.push(bundle);
                        self.redo_stack.clear();

                        self.top_panel = make_top_panel(ctx, app, true, false);
                        self.change_stage(ctx, app, 0);

                        return Transition::Push(PopupMsg::new_state(
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
                "Cancel" => {
                    if BundleEdits::get_current(app, &self.members) == self.original {
                        self.original.apply(app);
                        return Transition::Pop;
                    }
                    let original = self.original.clone();
                    return Transition::Push(ConfirmDiscard::new_state(
                        ctx,
                        Box::new(move |app| {
                            original.apply(app);
                        }),
                    ));
                }
                "Edit multiple signals" => {
                    // First commit the current changes, so we enter SignalPicker with clean state.
                    // This UX flow is a little unintuitive.
                    let changes = check_for_missing_turns(app, &self.members)
                        .unwrap_or_else(|| BundleEdits::get_current(app, &self.members));
                    self.original.apply(app);
                    changes.commit(ctx, app);
                    return Transition::Replace(picker::SignalPicker::new_state(
                        ctx,
                        self.members.clone(),
                        self.mode.clone(),
                    ));
                }
                "Export" => {
                    for signal in BundleEdits::get_current(app, &self.members).signals {
                        let ts = signal.export(&app.primary.map);
                        abstio::write_json(
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
            }
        }

        {
            if self.current_stage != 0 && ctx.input.pressed(Key::LeftArrow) {
                self.change_stage(ctx, app, self.current_stage - 1);
            }

            if self.current_stage != num_stages - 1 && ctx.input.pressed(Key::RightArrow) {
                self.change_stage(ctx, app, self.current_stage + 1);
            }
        }

        if ctx.redo_mouseover() {
            let old = self.movement_selected;

            self.movement_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for m in &self.movements {
                    let signal = app.primary.map.get_traffic_signal(m.id.parent);
                    if m.hitbox.contains_pt(pt) {
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
                self.change_stage(ctx, app, self.current_stage);
            }
        }

        if let Some((id, Some(pri))) = self.movement_selected {
            let signal = app.primary.map.get_traffic_signal(id.parent);
            let mut txt = Text::new();
            txt.add_line(Line(format!(
                "{} {}",
                match signal.stages[self.current_stage].get_priority_of_movement(id) {
                    TurnPriority::Protected => "Protected",
                    TurnPriority::Yield => "Yielding",
                    TurnPriority::Banned => "Forbidden",
                },
                if id.crosswalk { "crosswalk" } else { "turn" },
            )));
            txt.add_appended(vec![
                Line("Click").fg(ctx.style().text_hotkey_color),
                Line(format!(
                    " to {}",
                    match pri {
                        TurnPriority::Protected => "add it as protected",
                        TurnPriority::Yield => "allow it after yielding",
                        TurnPriority::Banned => "forbid it",
                    }
                )),
            ]);
            self.tooltip = Some(txt);
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
            app.draw(g, opts, &ShowEverything::new());
        }
        g.redraw(&self.fade_irrelevant);
        g.redraw(&self.draw_current);

        self.top_panel.draw(g);
        self.side_panel.draw(g);

        if let Some((id, _)) = self.movement_selected {
            let osd = if id.crosswalk {
                Text::from(format!(
                    "Crosswalk across {}",
                    app.primary
                        .map
                        .get_r(id.from.id)
                        .get_name(app.opts.language.as_ref())
                ))
            } else {
                Text::from(format!(
                    "Turn from {} to {}",
                    app.primary
                        .map
                        .get_r(id.from.id)
                        .get_name(app.opts.language.as_ref()),
                    app.primary
                        .map
                        .get_r(id.to.id)
                        .get_name(app.opts.language.as_ref())
                ))
            };
            CommonState::draw_custom_osd(g, app, osd);
        } else {
            CommonState::draw_osd(g, app);
        }

        if let Some(txt) = self.tooltip.clone() {
            g.draw_mouse_tooltip(txt);
        }
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App, can_undo: bool, can_redo: bool) -> Panel {
    let row = vec![
        ctx.style()
            .btn_solid_primary
            .text("Finish")
            .hotkey(Key::Enter)
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .text("Preview")
            .hotkey(lctrl(Key::P))
            .build_def(ctx),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/undo.svg")
            .disabled(!can_undo)
            .hotkey(lctrl(Key::Z))
            .build_widget(ctx, "undo"),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/redo.svg")
            .disabled(!can_redo)
            // TODO ctrl+shift+Z!
            .hotkey(lctrl(Key::Y))
            .build_widget(ctx, "redo"),
        ctx.style()
            .btn_plain_destructive
            .text("Cancel")
            .hotkey(Key::Escape)
            .build_def(ctx)
            .align_right(),
    ];
    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line("Traffic signal editor")
                .small_heading()
                .into_widget(ctx),
            ctx.style()
                .btn_plain
                .text("+ Edit multiple")
                .label_color(Color::hex("#4CA7E9"), ControlState::Default)
                .hotkey(Key::M)
                .build_widget(ctx, "Edit multiple signals"),
        ]),
        Widget::row(row),
        if app.opts.dev {
            ctx.style()
                .btn_outline
                .text("Export")
                .tooltip(Text::from_multiline(vec![
                    Line("This will create a JSON file in traffic_signal_data/.").small(),
                    Line(
                        "Contribute this to map how this traffic signal is currently timed in \
                         real life.",
                    )
                    .small(),
                ]))
                .build_def(ctx)
        } else {
            Widget::nothing()
        },
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_side_panel(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    selected: usize,
) -> Panel {
    let map = &app.primary.map;
    // Use any member for stage duration
    let canonical_signal = map.get_traffic_signal(*members.iter().next().unwrap());

    let mut txt = Text::new();
    if members.len() == 1 {
        let i = *members.iter().next().unwrap();
        txt.add_line(Line(i.to_string()).big_heading_plain());

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
            txt.add_line(Line(format!("  {}", r)).secondary());
        }
    } else {
        txt.add_line(Line(format!("{} intersections", members.len())).big_heading_plain());
        txt.add_line(
            Line(
                members
                    .iter()
                    .map(|i| format!("#{}", i.0))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .secondary(),
        );
    }
    let mut col = vec![txt.into_widget(ctx)];

    // Stage controls
    col.push(
        Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!(
                    "../../../../widgetry/icons/arrow_left.svg"
                ))
                .disabled(selected == 0)
                .build_widget(ctx, "move stage left"),
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!(
                    "../../../../widgetry/icons/arrow_right.svg"
                ))
                .disabled(selected == canonical_signal.stages.len() - 1)
                .build_widget(ctx, "move stage right"),
            match canonical_signal.stages[selected].stage_type {
                StageType::Fixed(d) => format!("Stage duration: {}", d),
                StageType::Variable(min, delay, additional) => format!(
                    "Stage duration: {}, {}, {} (variable)",
                    min, delay, additional
                ),
            }
            .text_widget(ctx)
            .centered_vert(),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/pencil.svg")
                .hotkey(Key::X)
                .build_widget(ctx, "change duration"),
            if canonical_signal.stages.len() > 1 {
                ctx.style()
                    .btn_solid_destructive
                    .icon("system/assets/tools/trash.svg")
                    .build_widget(ctx, "delete stage")
            } else {
                Widget::nothing()
            },
            ctx.style()
                .btn_plain
                .icon("system/assets/speed/plus.svg")
                .build_widget(ctx, "Add a new stage"),
        ])
        .padding(10)
        .bg(app.cs.inner_panel_bg),
    );

    let translations = squish_polygons_together(
        members
            .iter()
            .map(|i| app.primary.map.get_i(*i).polygon.clone())
            .collect(),
    );

    let mut stages_row = Vec::new();
    for idx in 0..canonical_signal.stages.len() {
        let stage_btn = Widget::col(vec![
            format!(
                "Stage {}: {}",
                idx + 1,
                match canonical_signal.stages[idx].stage_type {
                    StageType::Fixed(d) => format!("{}", d),
                    StageType::Variable(min, _, _) => format!("{} (v)", min),
                },
            )
            .text_widget(ctx),
            draw_multiple_signals(ctx, app, members, idx, &translations),
        ])
        .padding(10);
        // TODO Add a proper hover state to these buttons. Complication is that they're
        // MultiButtons...
        stages_row.push(if idx == selected {
            stage_btn.bg(ctx.style().btn_solid_primary.bg)
        } else {
            stage_btn
        });
    }
    col.push(
        Widget::row(stages_row)
            .padding(10)
            .bg(app.cs.inner_panel_bg),
    );

    col.push(Widget::row(vec![
        // TODO Say "normally" to account for variable stages?
        format!(
            "One full cycle lasts {}",
            canonical_signal.simple_cycle_duration()
        )
        .text_widget(ctx)
        .centered_vert(),
        if members.len() == 1 {
            ctx.style()
                .btn_outline
                .text("Edit entire signal")
                .hotkey(Key::E)
                .build_def(ctx)
        } else {
            ctx.style()
                .btn_outline
                .text("Tune offsets between signals")
                .hotkey(Key::O)
                .build_def(ctx)
        },
    ]));

    Panel::new_builder(Widget::col(col))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Center)
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
                signal.stages[idx].stage_type = canonical_stage.stage_type.clone();
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
    translations: &[(f64, f64)],
) -> Widget {
    let mut batch = GeomBatch::new();
    for (i, (dx, dy)) in members.iter().zip(translations) {
        let mut piece = GeomBatch::new();
        piece.push(
            app.cs.normal_intersection,
            app.primary.map.get_i(*i).polygon.clone(),
        );
        traffic_signal::draw_signal_stage(
            ctx.prerender,
            &app.primary.map.get_traffic_signal(*i).stages[idx],
            idx,
            *i,
            None,
            &mut piece,
            app,
            TrafficSignalStyle::Yuwen,
        );
        batch.append(piece.translate(*dx, *dy));
    }

    // Make the whole thing fit a fixed width
    let square_dims = 150.0;
    let bounds_before = batch.get_bounds();
    batch = batch.autocrop();
    let bounds = batch.get_bounds();
    let zoom = (square_dims / bounds.width()).min(square_dims / bounds.height());
    let batch = batch.scale(zoom);

    // Figure out the hitboxes per intersection, after all of these transformations
    let mut hitboxes = Vec::new();
    for (i, (dx, dy)) in members.iter().zip(translations) {
        hitboxes.push((
            app.primary
                .map
                .get_i(*i)
                .polygon
                .clone()
                .translate(*dx - bounds_before.min_x, *dy - bounds_before.min_y)
                .scale(zoom),
            format!("stage {}, {}", idx + 1, i),
        ));
    }
    MultiButton::new_widget(ctx, batch, hitboxes).named(format!("stage {}", idx + 1))
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
        let center = Pt2D::center(&polygons.iter().map(|p| p.center()).collect::<Vec<_>>());
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

pub fn fade_irrelevant(app: &App, members: &BTreeSet<IntersectionID>) -> GeomBatch {
    let mut holes = Vec::new();
    for i in members {
        let i = app.primary.map.get_i(*i);
        holes.push(i.polygon.clone());
        for r in &i.roads {
            holes.push(
                app.primary
                    .map
                    .get_r(*r)
                    .get_thick_polygon(&app.primary.map),
            );
        }
    }
    // The convex hull illuminates a bit more of the surrounding area, looks better
    let fade_area = Polygon::with_holes(
        app.primary.map.get_boundary_polygon().clone().into_ring(),
        vec![Polygon::convex_hull(holes).into_ring()],
    );
    GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)])
}
