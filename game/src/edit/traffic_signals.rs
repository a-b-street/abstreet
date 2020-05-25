use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::edit::{apply_map_edits, close_intersection, StopSignEditor};
use crate::game::{msg, DrawBaselayer, State, Transition, WizardState};
use crate::render::{
    draw_signal_phase, make_signal_diagram, DrawOptions, DrawTurnGroup, BIG_ARROW_THICKNESS,
};
use crate::sandbox::{spawn_agents_around, SpeedControls, TimePanel};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, RewriteColor, Text, TextExt, VerticalAlignment,
    Widget,
};
use geom::{ArrowCap, Duration};
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Phase,
    TurnGroupID, TurnPriority,
};
use std::collections::BTreeSet;

// TODO Warn if there are empty phases or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    pub i: IntersectionID,
    current_phase: usize,
    composite: Composite,
    pub top_panel: Composite,

    groups: Vec<DrawTurnGroup>,
    group_selected: Option<TurnGroupID>,

    // The first ControlTrafficSignal is the original
    pub command_stack: Vec<ControlTrafficSignal>,
    pub redo_stack: Vec<ControlTrafficSignal>,
}

impl TrafficSignalEditor {
    pub fn new(id: IntersectionID, ctx: &mut EventCtx, app: &mut App) -> TrafficSignalEditor {
        app.primary.current_selection = None;
        TrafficSignalEditor {
            i: id,
            current_phase: 0,
            composite: make_signal_diagram(ctx, app, id, 0, true),
            top_panel: make_top_panel(ctx, app, false, false),
            groups: DrawTurnGroup::for_i(id, &app.primary.map),
            group_selected: None,
            command_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn change_phase(&mut self, idx: usize, app: &App, ctx: &mut EventCtx) {
        if self.current_phase == idx {
            let mut new = make_signal_diagram(ctx, app, self.i, self.current_phase, true);
            new.restore(ctx, &self.composite);
            self.composite = new;
        } else {
            self.current_phase = idx;
            self.composite = make_signal_diagram(ctx, app, self.i, self.current_phase, true);
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
        if self.current_phase != 0 && ctx.input.new_was_pressed(&hotkey(Key::UpArrow).unwrap()) {
            self.change_phase(self.current_phase - 1, app, ctx);
        }

        if self.current_phase != app.primary.map.get_traffic_signal(self.i).phases.len() - 1
            && ctx.input.new_was_pressed(&hotkey(Key::DownArrow).unwrap())
        {
            self.change_phase(self.current_phase + 1, app, ctx);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x {
                x if x == "Edit entire signal" => {
                    return Transition::Push(edit_entire_signal(app, self.i));
                }
                x if x.starts_with("change duration of phase ") => {
                    let idx = x["change duration of phase ".len()..]
                        .parse::<usize>()
                        .unwrap()
                        - 1;
                    return Transition::Push(change_duration(app, self.i, idx));
                }
                x if x.starts_with("delete phase ") => {
                    let idx = x["delete phase ".len()..].parse::<usize>().unwrap() - 1;

                    let mut new_signal = orig_signal.clone();
                    new_signal.phases.remove(idx);
                    let num_phases = new_signal.phases.len();
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    // Don't use change_phase; it tries to preserve scroll
                    self.current_phase = if idx == num_phases { idx - 1 } else { idx };
                    self.composite =
                        make_signal_diagram(ctx, app, self.i, self.current_phase, true);
                    return Transition::Keep;
                }
                x if x.starts_with("move up phase ") => {
                    let idx = x["move up phase ".len()..].parse::<usize>().unwrap() - 1;

                    let mut new_signal = orig_signal.clone();
                    new_signal.phases.swap(idx, idx - 1);
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    self.change_phase(idx - 1, app, ctx);
                    return Transition::Keep;
                }
                x if x.starts_with("move down phase ") => {
                    let idx = x["move down phase ".len()..].parse::<usize>().unwrap() - 1;

                    let mut new_signal = orig_signal.clone();
                    new_signal.phases.swap(idx, idx + 1);
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    self.change_phase(idx + 1, app, ctx);
                    return Transition::Keep;
                }
                x if x == "Add new phase" => {
                    let mut new_signal = orig_signal.clone();
                    new_signal.phases.push(Phase::new());
                    let len = new_signal.phases.len();
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    self.change_phase(len - 1, app, ctx);
                    return Transition::Keep;
                }
                x if x.starts_with("phase ") => {
                    let idx = x["phase ".len()..].parse::<usize>().unwrap() - 1;
                    self.change_phase(idx, app, ctx);
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if ctx.redo_mouseover() {
            self.group_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for g in &self.groups {
                    if g.block.contains_pt(pt) {
                        self.group_selected = Some(g.id);
                        break;
                    }
                }
            }
        }

        if let Some(id) = self.group_selected {
            let mut new_signal = orig_signal.clone();
            let phase = &mut new_signal.phases[self.current_phase];
            // Just one key to toggle between the 3 states
            let next_priority = match phase.get_priority_of_group(id) {
                TurnPriority::Banned => {
                    if phase.could_be_protected(id, &orig_signal.turn_groups) {
                        Some(TurnPriority::Protected)
                    } else if id.crosswalk {
                        None
                    } else {
                        Some(TurnPriority::Yield)
                    }
                }
                TurnPriority::Yield => Some(TurnPriority::Banned),
                TurnPriority::Protected => {
                    if id.crosswalk {
                        Some(TurnPriority::Banned)
                    } else {
                        Some(TurnPriority::Yield)
                    }
                }
            };
            if let Some(pri) = next_priority {
                if app.per_obj.left_click(
                    ctx,
                    format!(
                        "toggle from {:?} to {:?}",
                        phase.get_priority_of_group(id),
                        pri
                    ),
                ) {
                    phase.edit_group(&orig_signal.turn_groups[&id], pri);
                    self.command_stack.push(orig_signal.clone());
                    self.redo_stack.clear();
                    self.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    self.change_phase(self.current_phase, app, ctx);
                    return Transition::Keep;
                }
            }
        }

        match self.top_panel.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Finish" => {
                    return check_for_missing_groups(
                        orig_signal.clone(),
                        &mut self.composite,
                        app,
                        ctx,
                    );
                }
                "Export" => {
                    let ts = orig_signal.export(&app.primary.map);
                    abstutil::write_json(
                        format!(
                            "../traffic_signal_data/{}.json",
                            ts.intersection_osm_node_id
                        ),
                        &ts,
                    );
                }
                "Preview" => {
                    // Might have to do this first!
                    app.primary
                        .map
                        .recalculate_pathfinding_after_edits(&mut Timer::throwaway());

                    return Transition::Push(make_previewer(self.i, self.current_phase));
                }
                "undo" => {
                    self.redo_stack.push(orig_signal.clone());
                    change_traffic_signal(self.command_stack.pop().unwrap(), app, ctx);
                    self.top_panel = make_top_panel(ctx, app, !self.command_stack.is_empty(), true);
                    self.change_phase(0, app, ctx);
                    return Transition::Keep;
                }
                "redo" => {
                    self.command_stack.push(orig_signal.clone());
                    change_traffic_signal(self.redo_stack.pop().unwrap(), app, ctx);
                    self.top_panel = make_top_panel(ctx, app, true, !self.redo_stack.is_empty());
                    self.change_phase(0, app, ctx);
                    return Transition::Keep;
                }
                _ => unreachable!(),
            },
            None => {}
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

        let signal = app.primary.map.get_traffic_signal(self.i);
        let phase = &signal.phases[self.current_phase];
        let mut batch = GeomBatch::new();
        draw_signal_phase(
            g.prerender,
            phase,
            self.i,
            None,
            &mut batch,
            app,
            app.opts.traffic_signal_style.clone(),
        );

        for g in &self.groups {
            if Some(g.id) == self.group_selected {
                batch.push(app.cs.selected, g.block.clone());
                // Overwrite the original thing
                batch.push(
                    app.cs.selected,
                    signal.turn_groups[&g.id]
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
                        .unwrap(),
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, g.block.clone());
            }
            let arrow_color = match phase.get_priority_of_group(g.id) {
                TurnPriority::Protected => app.cs.signal_protected_turn,
                TurnPriority::Yield => app.cs.signal_permitted_turn.alpha(1.0),
                TurnPriority::Banned => app.cs.signal_banned_turn,
            };
            batch.push(arrow_color, g.arrow.clone());
        }
        batch.draw(g);

        self.composite.draw(g);
        self.top_panel.draw(g);
        if let Some(id) = self.group_selected {
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
            CommonState::draw_osd(g, app, &None);
        }
    }
}

pub fn make_top_panel(ctx: &mut EventCtx, app: &App, can_undo: bool, can_redo: bool) -> Composite {
    let row = vec![
        Btn::text_fg("Finish")
            .build_def(ctx, hotkey(Key::Escape))
            .margin_right(5),
        Btn::text_fg("Preview").build_def(ctx, lctrl(Key::P)),
        (if can_undo {
            Btn::svg_def("../data/system/assets/tools/undo.svg").build(ctx, "undo", lctrl(Key::Z))
        } else {
            Widget::draw_svg_transform(
                ctx,
                "../data/system/assets/tools/undo.svg",
                RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
            )
        })
        .margin(15),
        (if can_redo {
            Btn::svg_def("../data/system/assets/tools/redo.svg").build(
                ctx,
                "redo",
                // TODO ctrl+shift+Z!
                lctrl(Key::Y),
            )
        } else {
            Widget::draw_svg_transform(
                ctx,
                "../data/system/assets/tools/redo.svg",
                RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
            )
        })
        .margin(15),
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
    Composite::new(Widget::row(row).bg(app.cs.panel_bg).padding(5))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
}

pub fn change_traffic_signal(signal: ControlTrafficSignal, app: &mut App, ctx: &mut EventCtx) {
    let mut edits = app.primary.map.get_edits().clone();
    // TODO Only record one command for the entire session. Otherwise, we can exit this editor and
    // undo a few times, potentially ending at an invalid state!
    let old = if let Some(prev) = edits.commands.last().and_then(|cmd| match cmd {
        EditCmd::ChangeIntersection {
            i,
            ref new,
            ref old,
        } => {
            if signal.id == *i {
                match new {
                    EditIntersection::TrafficSignal(_) => Some(old.clone()),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }) {
        edits.commands.pop();
        prev
    } else {
        app.primary.map.get_i_edit(signal.id)
    };
    edits.commands.push(EditCmd::ChangeIntersection {
        i: signal.id,
        old,
        new: EditIntersection::TrafficSignal(signal),
    });
    apply_map_edits(ctx, app, edits);
}

fn edit_entire_signal(app: &App, i: IntersectionID) -> Box<dyn State> {
    let has_sidewalks = app
        .primary
        .map
        .get_turns_in_intersection(i)
        .iter()
        .any(|t| t.between_sidewalks());
    let current_offset = app.primary.map.get_traffic_signal(i).offset;

    WizardState::new(Box::new(move |wiz, ctx, app| {
        let use_template = "use template";
        let all_walk = "add an all-walk phase at the end";
        let stop_sign = "convert to stop signs";
        let close = "close intersection for construction";
        let offset = "edit signal offset";
        let reset = "reset to default";

        let mut choices = vec![use_template, all_walk, stop_sign, close, offset, reset];
        if !has_sidewalks {
            choices.remove(1);
        }

        let mut wizard = wiz.wrap(ctx);
        match wizard.choose_string("", move || choices.clone())?.as_str() {
            x if x == use_template => {
                let (_, new_signal) =
                    wizard.choose("Use which preset for this intersection?", || {
                        Choice::from(ControlTrafficSignal::get_possible_policies(
                            &app.primary.map,
                            i,
                        ))
                    })?;
                Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                    editor
                        .command_stack
                        .push(app.primary.map.get_traffic_signal(editor.i).clone());
                    editor.redo_stack.clear();
                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    editor.change_phase(0, app, ctx);
                })))
            }
            x if x == all_walk => {
                Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                    let orig_signal = app.primary.map.get_traffic_signal(editor.i);
                    let mut new_signal = orig_signal.clone();
                    if new_signal.convert_to_ped_scramble() {
                        editor.command_stack.push(orig_signal.clone());
                        editor.redo_stack.clear();
                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        change_traffic_signal(new_signal, app, ctx);
                        editor.change_phase(0, app, ctx);
                    }
                })))
            }
            x if x == stop_sign => {
                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i,
                    old: app.primary.map.get_i_edit(i),
                    new: EditIntersection::StopSign(ControlStopSign::new(&app.primary.map, i)),
                });
                apply_map_edits(ctx, app, edits);
                Some(Transition::PopThenReplace(Box::new(StopSignEditor::new(
                    i, ctx, app,
                ))))
            }
            x if x == close => Some(close_intersection(ctx, app, i, false)),
            x if x == offset => {
                let new_duration = wizard.input_usize_prefilled(
                    "What should the offset of this traffic signal be (seconds)?",
                    format!("{}", current_offset.inner_seconds() as usize),
                )?;
                Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                    let mut signal = app.primary.map.get_traffic_signal(editor.i).clone();
                    editor.command_stack.push(signal.clone());
                    editor.redo_stack.clear();
                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    signal.offset = Duration::seconds(new_duration as f64);
                    change_traffic_signal(signal, app, ctx);
                    editor.change_phase(editor.current_phase, app, ctx);
                })))
            }
            x if x == reset => {
                Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                    let orig_signal = app.primary.map.get_traffic_signal(editor.i);
                    let new_signal =
                        ControlTrafficSignal::get_possible_policies(&app.primary.map, editor.i)
                            .remove(0)
                            .1;
                    editor.command_stack.push(orig_signal.clone());
                    editor.redo_stack.clear();
                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    change_traffic_signal(new_signal, app, ctx);
                    // Don't use change_phase; it tries to preserve scroll
                    editor.current_phase = 0;
                    editor.composite =
                        make_signal_diagram(ctx, app, editor.i, editor.current_phase, true);
                })))
            }
            _ => unreachable!(),
        }
    }))
}

fn change_duration(app: &App, i: IntersectionID, idx: usize) -> Box<dyn State> {
    let current_duration = app.primary.map.get_traffic_signal(i).phases[idx].duration;

    WizardState::new(Box::new(move |wiz, ctx, _| {
        let new_duration = wiz.wrap(ctx).input_something(
            "How long should this phase be (seconds)?",
            Some(format!("{}", current_duration.inner_seconds() as usize)),
            Box::new(|line| {
                line.parse::<usize>()
                    .ok()
                    .and_then(|n| if n != 0 { Some(n) } else { None })
            }),
        )?;
        Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
            let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
            let orig_signal = app.primary.map.get_traffic_signal(editor.i);

            let mut new_signal = orig_signal.clone();
            new_signal.phases[idx].duration = Duration::seconds(new_duration as f64);
            editor.command_stack.push(orig_signal.clone());
            editor.redo_stack.clear();
            editor.top_panel = make_top_panel(ctx, app, true, false);
            change_traffic_signal(new_signal, app, ctx);
            editor.change_phase(idx, app, ctx);
        })))
    }))
}

fn check_for_missing_groups(
    mut signal: ControlTrafficSignal,
    composite: &mut Composite,
    app: &mut App,
    ctx: &mut EventCtx,
) -> Transition {
    let mut missing: BTreeSet<TurnGroupID> = signal.turn_groups.keys().cloned().collect();
    for phase in &signal.phases {
        for g in &phase.protected_groups {
            missing.remove(g);
        }
        for g in &phase.yield_groups {
            missing.remove(g);
        }
    }
    if missing.is_empty() {
        let i = signal.id;
        if let Err(err) = signal.validate() {
            panic!("Edited traffic signal {} finalized with errors: {}", i, err);
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
    signal.phases.push(phase);
    let last_phase = signal.phases.len() - 1;
    let id = signal.id;
    change_traffic_signal(signal, app, ctx);
    *composite = make_signal_diagram(ctx, app, id, last_phase, true);

    Transition::Push(msg(
        "Error: missing turns",
        vec![
            format!("{} turns are missing from this traffic signal", num_missing),
            "They've all been added as a new last phase. Please update your changes to include \
             them."
                .to_string(),
        ],
    ))
}

// TODO I guess it's valid to preview without all turns possible. Some agents are just sad.
fn make_previewer(i: IntersectionID, phase: usize) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let random = "random agents around just this intersection".to_string();
        let right_now = format!(
            "change the traffic signal live at {}",
            app.suspended_sim.as_ref().unwrap().time()
        );
        match wiz
            .wrap(ctx)
            .choose_string(
                "Preview the traffic signal with what kind of traffic?",
                || vec![random.clone(), right_now.clone()],
            )?
            .as_str()
        {
            x if x == random => {
                // Start at the current phase
                let signal = app.primary.map.get_traffic_signal(i);
                // TODO Use the offset correctly
                let mut step = Duration::ZERO;
                for idx in 0..phase {
                    step += signal.phases[idx].duration;
                }
                app.primary.sim.normal_step(&app.primary.map, step);

                spawn_agents_around(i, app);
            }
            x if x == right_now => {
                app.primary.sim = app.suspended_sim.as_ref().unwrap().clone();
            }
            _ => unreachable!(),
        };
        Some(Transition::Replace(Box::new(PreviewTrafficSignal::new(
            ctx, app,
        ))))
    }))
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
            composite: Composite::new(
                Widget::col(vec![
                    "Previewing traffic signal".draw_text(ctx),
                    Btn::text_fg("back to editing").build_def(ctx, hotkey(Key::Escape)),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
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
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "back to editing" => {
                    app.primary.clear_sim();
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        self.time_panel.event(ctx, app);
        // TODO Ideally here reset to midnight would jump back to when the preview started?
        if let Some(t) = self.speed.event(ctx, app, None) {
            return t;
        }
        if self.speed.is_paused() {
            Transition::Keep
        } else {
            Transition::KeepWithMode(EventLoopMode::Animation)
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
        self.speed.draw(g);
        self.time_panel.draw(g);
    }
}
