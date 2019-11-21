use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::render::{draw_signal_phase, DrawOptions, DrawTurn, TrafficSignalDiagram};
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, Choice, Color, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu};
use geom::Duration;
use map_model::{
    ControlTrafficSignal, EditCmd, IntersectionID, Phase, TurnID, TurnPriority, TurnType,
};
use std::collections::BTreeSet;

// TODO Warn if there are empty phases or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    menu: ModalMenu,
    icon_selected: Option<TurnID>,
    diagram: TrafficSignalDiagram,
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
                (hotkey(Key::Escape), "quit"),
            ],
            ctx,
        );
        TrafficSignalEditor {
            menu,
            icon_selected: None,
            diagram: TrafficSignalDiagram::new(id, 0, ui, ctx),
        }
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);
        self.diagram.event(ctx, &mut self.menu);

        if ctx.redo_mouseover() {
            self.icon_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for t in ui
                    .primary
                    .draw_map
                    .get_turns(self.diagram.i, &ui.primary.map)
                {
                    if t.contains_pt(pt) {
                        self.icon_selected = Some(t.id);
                        break;
                    }
                }
            }
        }

        let mut signal = ui.primary.map.get_traffic_signal(self.diagram.i).clone();

        if let Some(id) = self.icon_selected {
            let phase = &mut signal.phases[self.diagram.current_phase()];
            // Just one key to toggle between the 3 states
            let next_priority = match phase.get_priority(id) {
                TurnPriority::Banned => {
                    if ui.primary.map.get_t(id).turn_type == TurnType::Crosswalk {
                        if phase.could_be_protected_turn(id, &ui.primary.map) {
                            Some(TurnPriority::Protected)
                        } else {
                            None
                        }
                    } else {
                        Some(TurnPriority::Yield)
                    }
                }
                TurnPriority::Yield => {
                    if phase.could_be_protected_turn(id, &ui.primary.map) {
                        Some(TurnPriority::Protected)
                    } else {
                        Some(TurnPriority::Banned)
                    }
                }
                TurnPriority::Protected => Some(TurnPriority::Banned),
            };
            if let Some(pri) = next_priority {
                if ctx.input.contextual_action(
                    Key::Space,
                    format!("toggle from {:?} to {:?}", phase.get_priority(id), pri),
                ) {
                    phase.edit_turn(ui.primary.map.get_t(id), pri);
                    change_traffic_signal(signal, ui, ctx);
                    return Transition::Keep;
                }
            }
        }

        if self.menu.action("quit") {
            return check_for_missing_turns(signal, &mut self.diagram, ui, ctx);
        }

        if self.menu.action("change phase duration") {
            return Transition::Push(make_change_phase_duration(
                signal.phases[self.diagram.current_phase()].duration,
            ));
        } else if self.menu.action("choose a preset signal") {
            return Transition::Push(make_change_preset(self.diagram.i));
        } else if self.menu.action("reset to original") {
            signal = ControlTrafficSignal::get_possible_policies(&ui.primary.map, self.diagram.i)
                .remove(0)
                .1;
            change_traffic_signal(signal, ui, ctx);
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
            signal.phases.swap(current_phase, current_phase - 1);
            change_traffic_signal(signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase - 1, ui, ctx);
        } else if current_phase != signal.phases.len() - 1
            && self.menu.action("move current phase down")
        {
            signal.phases.swap(current_phase, current_phase + 1);
            change_traffic_signal(signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase + 1, ui, ctx);
        } else if signal.phases.len() > 1 && self.menu.action("delete current phase") {
            signal.phases.remove(current_phase);
            let num_phases = signal.phases.len();
            change_traffic_signal(signal, ui, ctx);
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
            signal.phases.insert(current_phase, Phase::new());
            change_traffic_signal(signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase, ui, ctx);
        } else if has_sidewalks && self.menu.action("add a new pedestrian scramble phase") {
            let mut phase = Phase::new();
            for t in ui.primary.map.get_turns_in_intersection(self.diagram.i) {
                if t.between_sidewalks() {
                    phase.edit_turn(t, TurnPriority::Protected);
                }
            }
            signal.phases.insert(current_phase, phase);
            change_traffic_signal(signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, current_phase, ui, ctx);
        } else if has_sidewalks
            && self
                .menu
                .action("convert to dedicated pedestrian scramble phase")
        {
            signal.convert_to_ped_scramble(&ui.primary.map);
            change_traffic_signal(signal, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, 0, ui, ctx);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details = Some(self.diagram.i);
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
        }

        let mut batch = GeomBatch::new();
        let ctx = ui.draw_ctx();
        let map = &ui.primary.map;
        let phase = &map.get_traffic_signal(self.diagram.i).phases[self.diagram.current_phase()];
        for t in &ui.primary.draw_map.get_turns(self.diagram.i, map) {
            let arrow_color = match phase.get_priority(t.id) {
                TurnPriority::Protected => ui.cs.get("turn protected by traffic signal"),
                TurnPriority::Yield => ui
                    .cs
                    .get("turn that can yield by traffic signal")
                    .alpha(1.0),
                TurnPriority::Banned => ui.cs.get_def("turn not in current phase", Color::BLACK),
            };
            t.draw_icon(
                &mut batch,
                &ctx.cs,
                arrow_color,
                self.icon_selected == Some(t.id),
            );
        }
        draw_signal_phase(phase, self.diagram.i, None, &mut batch, &ctx);
        if let Some(id) = self.icon_selected {
            DrawTurn::draw_dashed(
                map.get_t(id),
                &mut batch,
                ui.cs.get_def("selected turn", Color::RED),
            );
        }
        batch.draw(g);

        self.diagram.draw(g, &ctx);

        self.menu.draw(g);
        if let Some(t) = self.icon_selected {
            CommonState::draw_osd(g, ui, &Some(ID::Turn(t)));
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

fn make_change_phase_duration(current_duration: Duration) -> Box<dyn State> {
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

fn make_change_preset(i: IntersectionID) -> Box<dyn State> {
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

fn check_for_missing_turns(
    mut signal: ControlTrafficSignal,
    diagram: &mut TrafficSignalDiagram,
    ui: &mut UI,
    ctx: &mut EventCtx,
) -> Transition {
    let mut missing_turns: BTreeSet<TurnID> = ui
        .primary
        .map
        .get_i(signal.id)
        .turns
        .iter()
        .cloned()
        .collect();
    for phase in &signal.phases {
        for t in &phase.protected_turns {
            missing_turns.remove(t);
        }
        for t in &phase.yield_turns {
            missing_turns.remove(t);
        }
    }
    if missing_turns.is_empty() {
        let i = signal.id;
        if let Err(err) = signal.validate(&ui.primary.map) {
            panic!("Edited traffic signal {} finalized with errors: {}", i, err);
        }
        return Transition::Pop;
    }
    let num_missing = missing_turns.len();
    let mut phase = Phase::new();
    phase.yield_turns = missing_turns;
    for t in ui.primary.map.get_turns_in_intersection(signal.id) {
        if t.turn_type == TurnType::SharedSidewalkCorner {
            phase.protected_turns.insert(t.id);
        }
    }
    signal.phases.push(phase);
    let last_phase = signal.phases.len() - 1;
    change_traffic_signal(signal, ui, ctx);
    *diagram = TrafficSignalDiagram::new(diagram.i, last_phase, ui, ctx);

    Transition::Push(msg("Error: missing turns", vec![format!("{} turns are missing from this traffic signal", num_missing), "They've all been added as a new last phase. Please update your changes to include them.".to_string()]))
}
