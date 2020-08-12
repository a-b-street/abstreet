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
use geom::{ArrowCap, Distance, Duration, Polygon};
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Phase,
    PhaseType, TurnGroup, TurnGroupID, TurnPriority,
};
use std::collections::BTreeSet;

// TODO Warn if there are empty phases or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    i: IntersectionID,
    current_phase: usize,
    composite: Composite,
    top_panel: Composite,
    mode: GameplayMode,

    groups: Vec<DrawTurnGroup>,
    // And the next priority to toggle to
    group_selected: Option<(TurnGroupID, Option<TurnPriority>)>,

    // The first ControlTrafficSignal is the original
    command_stack: Vec<ControlTrafficSignal>,
    redo_stack: Vec<ControlTrafficSignal>,

    fade_irrelevant: Drawable,
}

impl TrafficSignalEditor {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        id: IntersectionID,
        mode: GameplayMode,
    ) -> Box<dyn State> {
        app.primary.current_selection = None;

        let map = &app.primary.map;
        let mut holes = vec![map.get_i(id).polygon.clone()];
        for r in &map.get_i(id).roads {
            holes.push(map.get_r(*r).get_thick_polygon(map));
        }
        // The convex hull illuminates a bit more of the surrounding area, looks better
        let fade_area = Polygon::with_holes(
            map.get_boundary_polygon().clone().into_ring(),
            vec![Polygon::convex_hull(holes).into_ring()],
        );

        Box::new(TrafficSignalEditor {
            i: id,
            current_phase: 0,
            composite: make_signal_diagram(ctx, app, id, 0),
            top_panel: make_top_panel(ctx, app, false, false),
            mode,
            groups: DrawTurnGroup::for_i(id, map),
            group_selected: None,
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
            fade_irrelevant: GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)]).upload(ctx),
        })
    }

    fn change_phase(&mut self, idx: usize, ctx: &mut EventCtx, app: &App) {
        if self.current_phase == idx {
            let mut new = make_signal_diagram(ctx, app, self.i, self.current_phase);
            new.restore(ctx, &self.composite);
            self.composite = new;
        } else {
            self.current_phase = idx;
            self.composite = make_signal_diagram(ctx, app, self.i, self.current_phase);
            // TODO Maybe center of previous member
            self.composite
                .scroll_to_member(ctx, format!("phase {}", idx + 1));
        }
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let orig_signal = app.primary.map.get_traffic_signal(self.i);

        ctx.canvas_movement();

        // TODO Buttons for these...
        if self.current_phase != 0 && ctx.input.key_pressed(Key::UpArrow) {
            self.change_phase(self.current_phase - 1, ctx, app);
        }

        if self.current_phase != app.primary.map.get_traffic_signal(self.i).phases.len() - 1
            && ctx.input.key_pressed(Key::DownArrow)
        {
            self.change_phase(self.current_phase + 1, ctx, app);
        }

        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Edit entire signal" => {
                    return Transition::Push(edit_entire_signal(
                        ctx,
                        app,
                        self.i,
                        self.mode.clone(),
                        self.command_stack.get(0).cloned(),
                    ));
                }
                "Change signal offset" => {
                    let mut new_signal = orig_signal.clone();
                    new_signal.offset = Duration::seconds(self.composite.spinner("offset") as f64);

                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    app.primary.map.incremental_edit_traffic_signal(new_signal);
                    self.change_phase(self.current_phase, ctx, app);
                    return Transition::Keep;
                }
                "Add new phase" => {
                    let mut new_signal = orig_signal.clone();
                    new_signal.phases.push(Phase::new());
                    let len = new_signal.phases.len();
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    app.primary.map.incremental_edit_traffic_signal(new_signal);
                    self.change_phase(len - 1, ctx, app);
                    return Transition::Keep;
                }
                x => {
                    if let Some(x) = x.strip_prefix("change duration of phase ") {
                        let idx = x.parse::<usize>().unwrap() - 1;
                        return Transition::Push(ChangeDuration::new(ctx, app, self.i, idx));
                    }
                    if let Some(x) = x.strip_prefix("delete phase ") {
                        let idx = x.parse::<usize>().unwrap() - 1;

                        let mut new_signal = orig_signal.clone();
                        new_signal.phases.remove(idx);
                        let num_phases = new_signal.phases.len();
                        self.command_stack.push(orig_signal.clone());
                        self.redo_stack.clear();
                        self.top_panel = make_top_panel(ctx, app, true, false);
                        app.primary.map.incremental_edit_traffic_signal(new_signal);
                        // Don't use change_phase; it tries to preserve scroll
                        self.current_phase = if idx == num_phases { idx - 1 } else { idx };
                        self.composite = make_signal_diagram(ctx, app, self.i, self.current_phase);
                        return Transition::Keep;
                    }
                    if let Some(x) = x.strip_prefix("move up phase ") {
                        let idx = x.parse::<usize>().unwrap() - 1;

                        let mut new_signal = orig_signal.clone();
                        new_signal.phases.swap(idx, idx - 1);
                        self.command_stack.push(orig_signal.clone());
                        self.redo_stack.clear();
                        self.top_panel = make_top_panel(ctx, app, true, false);
                        app.primary.map.incremental_edit_traffic_signal(new_signal);
                        self.change_phase(idx - 1, ctx, app);
                        return Transition::Keep;
                    }
                    if let Some(x) = x.strip_prefix("move down phase ") {
                        let idx = x.parse::<usize>().unwrap() - 1;

                        let mut new_signal = orig_signal.clone();
                        new_signal.phases.swap(idx, idx + 1);
                        self.command_stack.push(orig_signal.clone());
                        self.redo_stack.clear();
                        self.top_panel = make_top_panel(ctx, app, true, false);
                        app.primary.map.incremental_edit_traffic_signal(new_signal);
                        self.change_phase(idx + 1, ctx, app);
                        return Transition::Keep;
                    }
                    if let Some(x) = x.strip_prefix("phase ") {
                        let idx = x.parse::<usize>().unwrap() - 1;
                        self.change_phase(idx, ctx, app);
                        return Transition::Keep;
                    }
                    unreachable!()
                }
            },
            _ => {}
        }

        if ctx.redo_mouseover() {
            self.group_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for g in &self.groups {
                    if g.block.contains_pt(pt) {
                        let phase = &orig_signal.phases[self.current_phase];
                        let next_priority = match phase.get_priority_of_group(g.id) {
                            TurnPriority::Banned => {
                                if phase.could_be_protected(g.id, &orig_signal.turn_groups) {
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

        if let Some((id, next_priority)) = self.group_selected {
            if let Some(pri) = next_priority {
                if app.per_obj.left_click(
                    ctx,
                    format!(
                        "toggle from {:?} to {:?}",
                        orig_signal.phases[self.current_phase].get_priority_of_group(id),
                        pri
                    ),
                ) {
                    let mut new_signal = orig_signal.clone();
                    new_signal.phases[self.current_phase]
                        .edit_group(&orig_signal.turn_groups[&id], pri);
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    app.primary.map.incremental_edit_traffic_signal(new_signal);
                    self.change_phase(self.current_phase, ctx, app);
                    return Transition::KeepWithMouseover;
                }
            }
        }

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Finish" => {
                    if let Some(orig) = self.command_stack.get(0) {
                        return check_for_missing_groups(ctx, app, &mut self.composite, orig);
                    } else {
                        // No changes
                        return Transition::Pop;
                    }
                }
                "Export" => {
                    let ts = orig_signal.export(&app.primary.map);
                    abstutil::write_json(
                        format!("traffic_signal_data/{}.json", ts.intersection_osm_node_id),
                        &ts,
                    );
                }
                "Preview" => {
                    // Might have to do this first!
                    app.primary
                        .map
                        .recalculate_pathfinding_after_edits(&mut Timer::throwaway());

                    return Transition::Push(make_previewer(ctx, app, self.i, self.current_phase));
                }
                "undo" => {
                    self.redo_stack.push(orig_signal.clone());
                    app.primary
                        .map
                        .incremental_edit_traffic_signal(self.command_stack.pop().unwrap());
                    self.top_panel = make_top_panel(ctx, app, !self.command_stack.is_empty(), true);
                    self.change_phase(0, ctx, app);
                    return Transition::Keep;
                }
                "redo" => {
                    self.command_stack.push(orig_signal.clone());
                    app.primary
                        .map
                        .incremental_edit_traffic_signal(self.redo_stack.pop().unwrap());
                    self.top_panel = make_top_panel(ctx, app, true, !self.redo_stack.is_empty());
                    self.change_phase(0, ctx, app);
                    return Transition::Keep;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details.push(self.i);
            app.draw(g, opts, &app.primary.sim, &ShowEverything::new());
        }
        g.redraw(&self.fade_irrelevant);

        let signal = app.primary.map.get_traffic_signal(self.i);
        let phase = match self.group_selected {
            Some((id, _)) => {
                let mut p = signal.phases[self.current_phase].clone();
                p.edit_group(&signal.turn_groups[&id], TurnPriority::Banned);
                p
            }
            _ => signal.phases[self.current_phase].clone(),
        };
        let mut batch = GeomBatch::new();
        draw_signal_phase(
            g.prerender,
            &phase,
            self.i,
            None,
            &mut batch,
            app,
            app.opts.traffic_signal_style.clone(),
        );

        for g in &self.groups {
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
                let arrow_color = match phase.get_priority_of_group(g.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn,
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, g.arrow.clone());
            }
        }
        batch.draw(g);

        self.composite.draw(g);
        self.top_panel.draw(g);
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

pub fn make_top_panel(ctx: &mut EventCtx, app: &App, can_undo: bool, can_redo: bool) -> Composite {
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
    Composite::new(Widget::row(row))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
}

fn edit_entire_signal(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    mode: GameplayMode,
    orig_signal: Option<ControlTrafficSignal>,
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
                        editor
                            .command_stack
                            .push(app.primary.map.get_traffic_signal(editor.i).clone());
                        editor.redo_stack.clear();
                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        app.primary.map.incremental_edit_traffic_signal(new_signal);
                        editor.change_phase(0, ctx, app);
                    }))
                }),
            )),
            x if x == all_walk => Transition::PopWithData(Box::new(move |state, ctx, app| {
                let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                let orig_signal = app.primary.map.get_traffic_signal(editor.i);
                let mut new_signal = orig_signal.clone();
                if new_signal.convert_to_ped_scramble() {
                    editor.command_stack.push(orig_signal.clone());
                    editor.redo_stack.clear();
                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    app.primary.map.incremental_edit_traffic_signal(new_signal);
                    editor.change_phase(0, ctx, app);
                }
            })),
            x if x == stop_sign => {
                // First restore the original signal
                if let Some(ref orig) = orig_signal {
                    app.primary
                        .map
                        .incremental_edit_traffic_signal(orig.clone());
                }

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
                // First restore the original signal
                if let Some(ref orig) = orig_signal {
                    app.primary
                        .map
                        .incremental_edit_traffic_signal(orig.clone());
                }

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
            x if x == reset => {
                Transition::PopWithData(Box::new(move |state, ctx, app| {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                    let orig_signal = app.primary.map.get_traffic_signal(editor.i);
                    let new_signal = ControlTrafficSignal::get_possible_policies(
                        &app.primary.map,
                        editor.i,
                        &mut Timer::throwaway(),
                    )
                    .remove(0)
                    .1;
                    editor.command_stack.push(orig_signal.clone());
                    editor.redo_stack.clear();
                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    app.primary.map.incremental_edit_traffic_signal(new_signal);
                    // Don't use change_phase; it tries to preserve scroll
                    editor.current_phase = 0;
                    editor.composite =
                        make_signal_diagram(ctx, app, editor.i, editor.current_phase);
                }))
            }
            _ => unreachable!(),
        }),
    )
}

struct ChangeDuration {
    composite: Composite,
    idx: usize,
}

impl ChangeDuration {
    fn new(ctx: &mut EventCtx, app: &App, i: IntersectionID, idx: usize) -> Box<dyn State> {
        let current = app.primary.map.get_traffic_signal(i).phases[idx]
            .phase_type
            .clone();

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
                        let orig_signal = app.primary.map.get_traffic_signal(editor.i);

                        let mut new_signal = orig_signal.clone();
                        new_signal.phases[idx].phase_type = new_type;
                        editor.command_stack.push(orig_signal.clone());
                        editor.redo_stack.clear();
                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        app.primary.map.incremental_edit_traffic_signal(new_signal);
                        editor.change_phase(idx, ctx, app);
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

fn check_for_missing_groups(
    ctx: &mut EventCtx,
    app: &mut App,
    composite: &mut Composite,
    orig_signal: &ControlTrafficSignal,
) -> Transition {
    let mut new_signal = app.primary.map.get_traffic_signal(orig_signal.id).clone();

    let mut missing: BTreeSet<TurnGroupID> = new_signal.turn_groups.keys().cloned().collect();
    for phase in &new_signal.phases {
        for g in &phase.protected_groups {
            missing.remove(g);
        }
        for g in &phase.yield_groups {
            missing.remove(g);
        }
    }
    if missing.is_empty() {
        match new_signal.validate() {
            Ok(new_signal) => {
                app.primary
                    .map
                    .incremental_edit_traffic_signal(orig_signal.clone());

                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i: new_signal.id,
                    old: app.primary.map.get_i_edit(new_signal.id),
                    new: EditIntersection::TrafficSignal(new_signal.export(&app.primary.map)),
                });
                apply_map_edits(ctx, app, edits);
            }
            Err(err) => {
                panic!(
                    "Edited traffic signal {} finalized with errors: {}",
                    orig_signal.id, err
                );
            }
        }
        return Transition::Pop;
    }
    let num_missing = missing.len();
    let mut phase = Phase::new();
    for g in missing {
        if g.crosswalk {
            phase.protected_groups.insert(g);
        } else {
            phase.yield_groups.insert(g);
        }
    }
    new_signal.phases.insert(0, phase);
    let id = new_signal.id;
    app.primary.map.incremental_edit_traffic_signal(new_signal);
    *composite = make_signal_diagram(ctx, app, id, 0);

    Transition::Push(PopupMsg::new(
        ctx,
        "Error: missing turns",
        vec![
            format!("{} turns are missing from this traffic signal", num_missing),
            "They've all been added as a new first phase. Please update your changes to include \
             them."
                .to_string(),
        ],
    ))
}

// TODO I guess it's valid to preview without all turns possible. Some agents are just sad.
fn make_previewer(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    phase: usize,
) -> Box<dyn State> {
    let random = "random agents around just this intersection".to_string();
    let right_now = format!(
        "change the traffic signal live at {}",
        app.suspended_sim.as_ref().unwrap().time()
    );

    ChooseSomething::new(
        ctx,
        "Preview the traffic signal with what kind of traffic?",
        Choice::strings(vec![random, right_now]),
        Box::new(move |x, ctx, app| {
            if x == "random agents around just this intersection" {
                // Start at the current phase
                let signal = app.primary.map.get_traffic_signal(i);
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

                spawn_agents_around(i, app);
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

// TODO Show diagram, auto-sync the phase.
// TODO Auto quit after things are gone?
pub struct PreviewTrafficSignal {
    composite: Composite,
    speed: SpeedControls,
    time_panel: TimePanel,
}

impl PreviewTrafficSignal {
    pub fn new(ctx: &mut EventCtx, app: &App) -> PreviewTrafficSignal {
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

fn make_signal_diagram(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    selected: usize,
) -> Composite {
    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = app.primary.map.get_i(i).polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = 150.0 / bounds.width();
    let bbox = Polygon::rectangle(zoom * bounds.width(), zoom * bounds.height());

    let signal = app.primary.map.get_traffic_signal(i);
    let txt_widget = {
        let mut txt = Text::from(Line(i.to_string()).big_heading_plain());

        let mut road_names = BTreeSet::new();
        for r in &app.primary.map.get_i(i).roads {
            road_names.insert(app.primary.map.get_r(*r).get_name());
        }
        for r in road_names {
            // TODO The spacing is ignored, so use -
            txt.add(Line(format!("- {}", r)));
        }

        txt.add(Line(""));
        txt.add(Line(format!("{} phases", signal.phases.len())).small_heading());
        txt.add(Line(format!("Signal offset: {}", signal.offset)));
        {
            let mut total = Duration::ZERO;
            for p in &signal.phases {
                total += p.phase_type.simple_duration();
            }
            // TODO Say "normally" or something?
            txt.add(Line(format!("One cycle lasts {}", total)));
        }
        txt.draw(ctx)
    };
    let mut col = vec![
        txt_widget,
        Btn::text_bg2("Edit entire signal").build_def(ctx, hotkey(Key::E)),
    ];

    for (idx, phase) in signal.phases.iter().enumerate() {
        // Separator
        col.push(
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
                )]),
            )
            .centered_horiz(),
        );

        let phase_btn = {
            let mut orig_batch = GeomBatch::new();
            draw_signal_phase(
                ctx.prerender,
                phase,
                i,
                None,
                &mut orig_batch,
                app,
                TrafficSignalStyle::Sidewalks,
            );

            let mut normal = GeomBatch::new();
            normal.push(Color::BLACK, bbox.clone());
            normal.append(
                orig_batch
                    .translate(-bounds.min_x, -bounds.min_y)
                    .scale(zoom),
            );

            let mut hovered = GeomBatch::new();
            hovered.append(normal.clone());
            hovered.push(Color::RED, bbox.to_outline(Distance::meters(5.0)).unwrap());

            Btn::custom(normal, hovered, bbox.clone()).build(
                ctx,
                format!("phase {}", idx + 1),
                None,
            )
        };

        let phase_col = Widget::col(vec![
            Widget::row(vec![
                match phase.phase_type {
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
                if signal.phases.len() > 1 {
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
                    if idx == signal.phases.len() - 1 {
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

    // Separator
    col.push(
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(
                Color::WHITE,
                Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
            )]),
        )
        .centered_horiz(),
    );

    col.push(Btn::text_fg("Add new phase").build_def(ctx, None));
    col.push(Widget::row(vec![
        Spinner::new(ctx, (0, 300), signal.offset.inner_seconds() as isize).named("offset"),
        Btn::text_fg("Change signal offset").build_def(ctx, None),
    ]));

    Composite::new(Widget::col(col))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .exact_size_percent(30, 85)
        .build(ctx)
}

pub fn draw_selected_group(
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
