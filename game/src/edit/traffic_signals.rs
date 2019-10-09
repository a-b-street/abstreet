use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::render::{draw_signal_phase, DrawCtx, DrawOptions, DrawTurn, TrafficSignalDiagram};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{hotkey, Choice, Color, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu};
use geom::Duration;
use map_model::{ControlTrafficSignal, IntersectionID, Phase, TurnID, TurnPriority, TurnType};

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
                vec![
                    (hotkey(Key::UpArrow), "select previous phase"),
                    (hotkey(Key::DownArrow), "select next phase"),
                ],
                vec![
                    (hotkey(Key::D), "change phase duration"),
                    (hotkey(Key::K), "move current phase up"),
                    (hotkey(Key::J), "move current phase down"),
                    (hotkey(Key::Backspace), "delete current phase"),
                    (hotkey(Key::N), "add a new empty phase"),
                    (hotkey(Key::M), "add a new pedestrian scramble phase"),
                ],
                vec![
                    (hotkey(Key::R), "reset to original"),
                    (hotkey(Key::P), "choose a preset signal"),
                    (
                        hotkey(Key::B),
                        "convert to dedicated pedestrian scramble phase",
                    ),
                ],
                vec![(hotkey(Key::Escape), "quit")],
            ],
            ctx,
        );
        TrafficSignalEditor {
            menu,
            icon_selected: None,
            diagram: TrafficSignalDiagram::new(id, 0, &ui.primary.map, ctx),
        }
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
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
                        if phase.could_be_priority_turn(id, &ui.primary.map) {
                            Some(TurnPriority::Priority)
                        } else {
                            None
                        }
                    } else {
                        Some(TurnPriority::Yield)
                    }
                }
                TurnPriority::Stop => {
                    panic!("Can't have TurnPriority::Stop in a traffic signal");
                }
                TurnPriority::Yield => {
                    if phase.could_be_priority_turn(id, &ui.primary.map) {
                        Some(TurnPriority::Priority)
                    } else {
                        Some(TurnPriority::Banned)
                    }
                }
                TurnPriority::Priority => Some(TurnPriority::Banned),
            };
            if let Some(pri) = next_priority {
                if ctx.input.contextual_action(
                    Key::Space,
                    format!("toggle from {:?} to {:?}", phase.get_priority(id), pri),
                ) {
                    phase.edit_turn(ui.primary.map.get_t(id), pri);
                    change_traffic_signal(signal, self.diagram.i, ui, ctx);
                    return Transition::Keep;
                }
            }
        }

        if self.menu.action("quit") {
            return Transition::Pop;
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
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, 0, &ui.primary.map, ctx);
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
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram =
                TrafficSignalDiagram::new(self.diagram.i, current_phase - 1, &ui.primary.map, ctx);
        } else if current_phase != signal.phases.len() - 1
            && self.menu.action("move current phase down")
        {
            signal.phases.swap(current_phase, current_phase + 1);
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram =
                TrafficSignalDiagram::new(self.diagram.i, current_phase + 1, &ui.primary.map, ctx);
        } else if signal.phases.len() > 1 && self.menu.action("delete current phase") {
            signal.phases.remove(current_phase);
            let num_phases = signal.phases.len();
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(
                self.diagram.i,
                if current_phase == num_phases {
                    current_phase - 1
                } else {
                    current_phase
                },
                &ui.primary.map,
                ctx,
            );
        } else if self.menu.action("add a new empty phase") {
            signal
                .phases
                .insert(current_phase, Phase::new(self.diagram.i));
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram =
                TrafficSignalDiagram::new(self.diagram.i, current_phase, &ui.primary.map, ctx);
        } else if has_sidewalks && self.menu.action("add a new pedestrian scramble phase") {
            let mut phase = Phase::new(self.diagram.i);
            for t in ui.primary.map.get_turns_in_intersection(self.diagram.i) {
                if t.between_sidewalks() {
                    phase.edit_turn(t, TurnPriority::Priority);
                }
            }
            signal.phases.insert(current_phase, phase);
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram =
                TrafficSignalDiagram::new(self.diagram.i, current_phase, &ui.primary.map, ctx);
        } else if has_sidewalks
            && self
                .menu
                .action("convert to dedicated pedestrian scramble phase")
        {
            signal.convert_to_ped_scramble(&ui.primary.map);
            change_traffic_signal(signal, self.diagram.i, ui, ctx);
            self.diagram = TrafficSignalDiagram::new(self.diagram.i, 0, &ui.primary.map, ctx);
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
        let ctx = DrawCtx {
            cs: &ui.cs,
            map: &ui.primary.map,
            draw_map: &ui.primary.draw_map,
            sim: &ui.primary.sim,
        };
        let map = &ui.primary.map;
        let phase = &map.get_traffic_signal(self.diagram.i).phases[self.diagram.current_phase()];
        for t in &ui.primary.draw_map.get_turns(self.diagram.i, map) {
            let arrow_color = match phase.get_priority(t.id) {
                TurnPriority::Priority => ui
                    .cs
                    .get_def("priority turn in current phase", Color::GREEN),
                TurnPriority::Yield => ui
                    .cs
                    .get_def("yield turn in current phase", Color::rgb(255, 105, 180)),
                TurnPriority::Banned => ui.cs.get_def("turn not in current phase", Color::BLACK),
                TurnPriority::Stop => panic!("Can't have TurnPriority::Stop in a traffic signal"),
            };
            t.draw_icon(
                &mut batch,
                &ctx.cs,
                arrow_color,
                self.icon_selected == Some(t.id),
            );
        }
        draw_signal_phase(phase, None, &mut batch, &ctx);
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

fn change_traffic_signal(
    signal: ControlTrafficSignal,
    i: IntersectionID,
    ui: &mut UI,
    ctx: &mut EventCtx,
) {
    let orig = ControlTrafficSignal::new(&ui.primary.map, i, &mut Timer::throwaway());
    let mut new_edits = ui.primary.map.get_edits().clone();
    if orig == signal {
        new_edits.traffic_signal_overrides.remove(&i);
    } else {
        new_edits.traffic_signal_overrides.insert(i, signal);
    }
    apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
}

fn make_change_phase_duration(current_duration: Duration) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let new_duration = wiz.wrap(ctx).input_usize_prefilled(
            "How long should this phase be?",
            format!("{}", current_duration.inner_seconds() as usize),
        )?;
        Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
            let mut editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
            let mut signal = ui.primary.map.get_traffic_signal(editor.diagram.i).clone();
            let idx = editor.diagram.current_phase();
            signal.phases[idx].duration = Duration::seconds(new_duration as f64);
            change_traffic_signal(signal, editor.diagram.i, ui, ctx);
            editor.diagram = TrafficSignalDiagram::new(editor.diagram.i, idx, &ui.primary.map, ctx);
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
            change_traffic_signal(new_signal, editor.diagram.i, ui, ctx);
            editor.diagram = TrafficSignalDiagram::new(editor.diagram.i, 0, &ui.primary.map, ctx);
        })))
    }))
}
