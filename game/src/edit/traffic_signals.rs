use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{msg, State, Transition, WizardState};
use crate::render::{
    draw_signal_phase, DrawOptions, DrawTurnGroup, TrafficSignalDiagram, BIG_ARROW_THICKNESS,
};
use crate::sandbox::spawn_agents_around;
use crate::ui::{ShowEverything, UI};
use abstutil::{elapsed_seconds, Timer};
use ezgui::{
    hotkey, lctrl, Choice, Color, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, Line, ModalMenu,
    Text,
};
use geom::Duration;
use map_model::{
    ControlTrafficSignal, EditCmd, IntersectionID, Phase, TurnGroupID, TurnPriority, TurnType,
};
use std::collections::BTreeSet;
use std::time::Instant;

// TODO Warn if there are empty phases or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    menu: ModalMenu,
    diagram: TrafficSignalDiagram,
    groups: Vec<DrawTurnGroup>,
    group_selected: Option<TurnGroupID>,
}

impl TrafficSignalEditor {
    pub fn new(id: IntersectionID, ctx: &mut EventCtx, ui: &mut UI) -> TrafficSignalEditor {
        ui.primary.current_selection = None;
        let menu = ModalMenu::new(
            &format!("Traffic Signal Editor for {}", id),
            vec![
                (hotkey(Key::UpArrow), "select previous phase"),
                (hotkey(Key::DownArrow), "select next phase"),
                (hotkey(Key::D), "change phase duration"),
                (hotkey(Key::K), "move current phase up"),
                (hotkey(Key::J), "move current phase down"),
                (hotkey(Key::Backspace), "delete current phase"),
                (hotkey(Key::N), "add a new empty phase"),
                (hotkey(Key::M), "add a new pedestrian scramble phase"),
                (hotkey(Key::R), "reset to original"),
                (hotkey(Key::P), "choose a preset signal"),
                (
                    hotkey(Key::B),
                    "convert to dedicated pedestrian scramble phase",
                ),
                (hotkey(Key::O), "change signal offset"),
                (lctrl(Key::P), "preview changes"),
                (hotkey(Key::Escape), "quit"),
            ],
            ctx,
        );
        TrafficSignalEditor {
            menu,
            diagram: TrafficSignalDiagram::new(id, 0, ui, ctx),
            groups: DrawTurnGroup::for_i(id, &ui.primary.map),
            group_selected: None,
        }
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let orig_signal = ui.primary.map.get_traffic_signal(self.diagram.i);

        self.menu.event(ctx);
        // TODO This really needs to be shown in the diagram!
        self.menu.set_info(
            ctx,
            Text::from(Line(format!(
                "Signal offset: {}",
                orig_signal.offset.minimal_tostring()
            ))),
        );
        ctx.canvas.handle_event(ctx.input);
        self.diagram.event(ctx, &mut self.menu);

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
            let phase = &mut new_signal.phases[self.diagram.current_phase()];
            // Just one key to toggle between the 3 states
            let next_priority = match phase.get_priority_of_group(id) {
                TurnPriority::Banned => {
                    if phase.could_be_protected(id, &orig_signal.turn_groups) {
                        Some(TurnPriority::Protected)
                    } else if id.crosswalk.is_some() {
                        None
                    } else {
                        Some(TurnPriority::Yield)
                    }
                }
                TurnPriority::Yield => Some(TurnPriority::Banned),
                TurnPriority::Protected => {
                    if id.crosswalk.is_some() {
                        Some(TurnPriority::Banned)
                    } else {
                        Some(TurnPriority::Yield)
                    }
                }
            };
            if let Some(pri) = next_priority {
                if ctx.input.contextual_action(
                    Key::Space,
                    format!(
                        "toggle from {:?} to {:?}",
                        phase.get_priority_of_group(id),
                        pri
                    ),
                ) {
                    phase.edit_group(
                        &orig_signal.turn_groups[&id],
                        pri,
                        &orig_signal.turn_groups,
                        &ui.primary.map,
                    );
                    change_traffic_signal(new_signal, ui, ctx);
                    return Transition::Keep;
                }
            }
        }

        if self.menu.action("quit") {
            return check_for_missing_groups(orig_signal.clone(), &mut self.diagram, ui, ctx);
        }

        if self.menu.action("change phase duration") {
            return Transition::Push(change_phase_duration(
                orig_signal.phases[self.diagram.current_phase()].duration,
            ));
        } else if self.menu.action("change signal offset") {
            return Transition::Push(change_offset(orig_signal.offset));
        } else if self.menu.action("choose a preset signal") {
            return Transition::Push(change_preset(self.diagram.i));
        } else if self.menu.action("reset to original") {
            let new_signal =
                ControlTrafficSignal::get_possible_policies(&ui.primary.map, self.diagram.i)
                    .remove(0)
                    .1;
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, 0, ui, ctx);
            return Transition::Keep;
        }

        let has_sidewalks = ui
            .primary
            .map
            .get_turns_in_intersection(self.diagram.i)
            .iter()
            .any(|t| t.between_sidewalks());

        let current_phase = self.diagram.current_phase();

        if current_phase != 0 && self.menu.action("move current phase up") {
            let mut new_signal = orig_signal.clone();
            new_signal.phases.swap(current_phase, current_phase - 1);
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase - 1, ui, ctx);
        } else if current_phase != orig_signal.phases.len() - 1
            && self.menu.action("move current phase down")
        {
            let mut new_signal = orig_signal.clone();
            new_signal.phases.swap(current_phase, current_phase + 1);
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase + 1, ui, ctx);
        } else if orig_signal.phases.len() > 1 && self.menu.action("delete current phase") {
            let mut new_signal = orig_signal.clone();
            new_signal.phases.remove(current_phase);
            let num_phases = new_signal.phases.len();
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(
                self.diagram.i,
                if current_phase == num_phases {
                    current_phase - 1
                } else {
                    current_phase
                },
                ui,
                ctx,
            );
        } else if self.menu.action("add a new empty phase") {
            let mut new_signal = orig_signal.clone();
            new_signal.phases.insert(current_phase + 1, Phase::new());
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase + 1, ui, ctx);
        } else if has_sidewalks && self.menu.action("add a new pedestrian scramble phase") {
            let mut phase = Phase::new();
            for g in orig_signal.turn_groups.values() {
                if g.turn_type == TurnType::Crosswalk {
                    phase.protected_groups.insert(g.id);
                }
            }
            let mut new_signal = orig_signal.clone();
            new_signal.phases.insert(current_phase + 1, phase);
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase + 1, ui, ctx);
        } else if has_sidewalks
            && self
                .menu
                .action("convert to dedicated pedestrian scramble phase")
        {
            let mut new_signal = orig_signal.clone();
            new_signal.convert_to_ped_scramble(&ui.primary.map);
            change_traffic_signal(new_signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, 0, ui, ctx);
        }

        if self.menu.action("preview changes") {
            return Transition::PushWithMode(
                Box::new(PreviewTrafficSignal::new(
                    self.diagram.i,
                    current_phase,
                    ui,
                    ctx,
                )),
                EventLoopMode::Animation,
            );
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details = Some(self.diagram.i);
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
        }

        let signal = ui.primary.map.get_traffic_signal(self.diagram.i);
        let phase = &signal.phases[self.diagram.current_phase()];
        let ctx = ui.draw_ctx();
        let mut batch = GeomBatch::new();
        draw_signal_phase(phase, self.diagram.i, None, &mut batch, &ctx);

        for g in &self.groups {
            if Some(g.id) == self.group_selected {
                batch.push(ui.cs.get_def("solid selected", Color::RED), g.block.clone());
                // Overwrite the original thing
                batch.push(
                    ui.cs.get("solid selected"),
                    signal.turn_groups[&g.id]
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS)
                        .unwrap(),
                );
            } else {
                batch.push(
                    ui.cs.get_def("turn block background", Color::grey(0.6)),
                    g.block.clone(),
                );
            }
            let arrow_color = match phase.get_priority_of_group(g.id) {
                TurnPriority::Protected => ui.cs.get("turn protected by traffic signal"),
                TurnPriority::Yield => ui
                    .cs
                    .get("turn that can yield by traffic signal")
                    .alpha(1.0),
                TurnPriority::Banned => ui.cs.get_def("turn not in current phase", Color::BLACK),
            };
            batch.push(arrow_color, g.arrow.clone());
        }
        batch.draw(g);

        self.diagram.draw(g, &ctx);

        self.menu.draw(g);
        if let Some(id) = self.group_selected {
            let osd = if id.crosswalk.is_some() {
                Text::from(Line(format!(
                    "Crosswalk across {}",
                    ui.primary.map.get_r(id.from).get_name()
                )))
            } else {
                Text::from(Line(format!(
                    "Turn from {} to {}",
                    ui.primary.map.get_r(id.from).get_name(),
                    ui.primary.map.get_r(id.to).get_name()
                )))
            };
            CommonState::draw_custom_osd(g, osd);
        } else {
            CommonState::draw_osd(g, ui, &None);
        }
    }
}

fn change_traffic_signal(signal: ControlTrafficSignal, ui: &mut UI, ctx: &mut EventCtx) {
    let mut edits = ui.primary.map.get_edits().clone();
    // TODO Only record one command for the entire session. Otherwise, we can exit this editor and
    // undo a few times, potentially ending at an invalid state!
    if edits
        .commands
        .last()
        .map(|cmd| match cmd {
            EditCmd::ChangeTrafficSignal(ref s) => s.id == signal.id,
            _ => false,
        })
        .unwrap_or(false)
    {
        edits.commands.pop();
    }
    edits.commands.push(EditCmd::ChangeTrafficSignal(signal));
    apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
}

fn change_phase_duration(current_duration: Duration) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let new_duration = wiz.wrap(ctx).input_usize_prefilled(
            "How long should this phase be (seconds)?",
            format!("{}", current_duration.inner_seconds() as usize),
        )?;
        Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
            let mut editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
            let mut signal = ui.primary.map.get_traffic_signal(editor.diagram.i).clone();
            let idx = editor.diagram.current_phase();
            signal.phases[idx].duration = Duration::seconds(new_duration as f64);
            change_traffic_signal(signal, ui, ctx);
            editor.diagram = TrafficSignalDiagram::new(editor.diagram.i, idx, ui, ctx);
        })))
    }))
}

fn change_offset(current_duration: Duration) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let new_duration = wiz.wrap(ctx).input_usize_prefilled(
            "What should the offset of this traffic signal be (seconds)?",
            format!("{}", current_duration.inner_seconds() as usize),
        )?;
        Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
            let mut editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
            let mut signal = ui.primary.map.get_traffic_signal(editor.diagram.i).clone();
            signal.offset = Duration::seconds(new_duration as f64);
            change_traffic_signal(signal, ui, ctx);
            editor.diagram = TrafficSignalDiagram::new(
                editor.diagram.i,
                editor.diagram.current_phase(),
                ui,
                ctx,
            );
        })))
    }))
}

fn change_preset(i: IntersectionID) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let (_, new_signal) =
            wiz.wrap(ctx)
                .choose("Use which preset for this intersection?", || {
                    Choice::from(ControlTrafficSignal::get_possible_policies(
                        &ui.primary.map,
                        i,
                    ))
                })?;
        Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
            let mut editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
            change_traffic_signal(new_signal, ui, ctx);
            editor.diagram = TrafficSignalDiagram::new(editor.diagram.i, 0, ui, ctx);
        })))
    }))
}

fn check_for_missing_groups(
    mut signal: ControlTrafficSignal,
    diagram: &mut TrafficSignalDiagram,
    ui: &mut UI,
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
    phase.yield_groups = missing;
    signal.phases.push(phase);
    let last_phase = signal.phases.len() - 1;
    change_traffic_signal(signal, ui, ctx);
    *diagram = TrafficSignalDiagram::new(diagram.i, last_phase, ui, ctx);

    Transition::Push(msg("Error: missing turns", vec![format!("{} turns are missing from this traffic signal", num_missing), "They've all been added as a new last phase. Please update your changes to include them.".to_string()]))
}

// TODO Show diagram, auto-sync the phase.
// TODO Auto quit after things are gone?
struct PreviewTrafficSignal {
    menu: ModalMenu,
    last_step: Instant,
}

impl PreviewTrafficSignal {
    fn new(
        i: IntersectionID,
        phase_idx: usize,
        ui: &mut UI,
        ctx: &EventCtx,
    ) -> PreviewTrafficSignal {
        // Start at the current phase
        let signal = ui.primary.map.get_traffic_signal(i);
        // TODO Use the offset correctly
        let mut step = Duration::ZERO;
        for idx in 0..phase_idx {
            step += signal.phases[idx].duration;
        }
        ui.primary.sim.step(&ui.primary.map, step);

        // This should be a no-op
        ui.primary
            .map
            .recalculate_pathfinding_after_edits(&mut Timer::throwaway());
        spawn_agents_around(i, ui, ctx);

        PreviewTrafficSignal {
            menu: ModalMenu::new(
                "Preview traffic signal",
                vec![(hotkey(Key::Escape), "back to editing")],
                ctx,
            ),
            last_step: Instant::now(),
        }
    }
}

impl State for PreviewTrafficSignal {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);
        if self.menu.action("back to editing") {
            ui.primary.clear_sim();
            return Transition::Pop;
        }

        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            let dt = Duration::seconds(elapsed_seconds(self.last_step));
            self.last_step = Instant::now();
            ui.primary
                .sim
                .time_limited_step(&ui.primary.map, dt, Duration::seconds(0.1));
            self.menu.set_info(
                ctx,
                Text::from(Line(format!(
                    "Time: {}",
                    ui.primary.sim.time().minimal_tostring()
                ))),
            );
        }

        Transition::KeepWithMode(EventLoopMode::Animation)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}
