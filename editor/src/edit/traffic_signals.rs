use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::{draw_signal_cycle, draw_signal_diagram, DrawCtx, DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, Color, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu, MultiKey, Wizard, WrappedWizard,
};
use geom::Duration;
use map_model::{ControlTrafficSignal, Cycle, IntersectionID, Map, TurnID, TurnPriority, TurnType};

// TODO Warn if there are empty cycles or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    menu: ModalMenu,
    i: IntersectionID,
    current_cycle: usize,
    icon_selected: Option<TurnID>,
}

impl TrafficSignalEditor {
    pub fn new(id: IntersectionID, ctx: &mut EventCtx, ui: &mut UI) -> TrafficSignalEditor {
        ui.primary.current_selection = None;
        let menu = ModalMenu::new(
            &format!("Traffic Signal Editor for {}", id),
            vec![
                vec![
                    (hotkey(Key::UpArrow), "select previous cycle"),
                    (hotkey(Key::DownArrow), "select next cycle"),
                ],
                vec![
                    (hotkey(Key::D), "change cycle duration"),
                    (hotkey(Key::K), "move current cycle up"),
                    (hotkey(Key::J), "move current cycle down"),
                    (hotkey(Key::Backspace), "delete current cycle"),
                    (hotkey(Key::N), "add a new empty cycle"),
                    (hotkey(Key::M), "add a new pedestrian scramble cycle"),
                ],
                vec![
                    (hotkey(Key::R), "reset to original"),
                    (hotkey(Key::P), "choose a preset signal"),
                    (
                        hotkey(Key::B),
                        "convert to dedicated pedestrian scramble cycle",
                    ),
                ],
                vec![(hotkey(Key::Escape), "quit")],
            ],
            ctx,
        );
        TrafficSignalEditor {
            menu,
            i: id,
            current_cycle: 0,
            icon_selected: None,
        }
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        if ctx.redo_mouseover() {
            self.icon_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for t in ui.primary.draw_map.get_turns(self.i, &ui.primary.map) {
                    if t.contains_pt(pt) {
                        self.icon_selected = Some(t.id);
                        break;
                    }
                }
            }
        }

        let mut signal = ui.primary.map.get_traffic_signal(self.i).clone();

        if let Some(id) = self.icon_selected {
            let cycle = &mut signal.cycles[self.current_cycle];
            // Just one key to toggle between the 3 states
            let next_priority = match cycle.get_priority(id) {
                TurnPriority::Banned => {
                    if ui.primary.map.get_t(id).turn_type == TurnType::Crosswalk {
                        if cycle.could_be_priority_turn(id, &ui.primary.map) {
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
                    if cycle.could_be_priority_turn(id, &ui.primary.map) {
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
                    &format!("toggle from {:?} to {:?}", cycle.get_priority(id), pri),
                ) {
                    cycle.edit_turn(ui.primary.map.get_t(id), pri);
                    change_traffic_signal(signal, self.i, ui, ctx);
                    return Transition::Keep;
                }
            }
        }

        if self.menu.action("quit") {
            return Transition::Pop;
        }

        if self.current_cycle != 0 && self.menu.action("select previous cycle") {
            self.current_cycle -= 1;
        }
        if self.current_cycle != ui.primary.map.get_traffic_signal(self.i).cycles.len() - 1
            && self.menu.action("select next cycle")
        {
            self.current_cycle += 1;
        }

        if self.menu.action("change cycle duration") {
            return Transition::Push(Box::new(ChangeCycleDuration {
                cycle: signal.cycles[self.current_cycle].clone(),
                wizard: Wizard::new(),
            }));
        } else if self.menu.action("choose a preset signal") {
            return Transition::Push(Box::new(ChangePreset {
                i: self.i,
                wizard: Wizard::new(),
            }));
        } else if self.menu.action("reset to original") {
            signal = ControlTrafficSignal::get_possible_policies(&ui.primary.map, self.i)
                .remove(0)
                .1;
            change_traffic_signal(signal, self.i, ui, ctx);
            self.current_cycle = 0;
            return Transition::Keep;
        }

        let has_sidewalks = ui
            .primary
            .map
            .get_turns_in_intersection(self.i)
            .iter()
            .any(|t| t.between_sidewalks());

        if self.current_cycle != 0 && self.menu.action("move current cycle up") {
            signal
                .cycles
                .swap(self.current_cycle, self.current_cycle - 1);
            change_traffic_signal(signal, self.i, ui, ctx);
            self.current_cycle -= 1;
        } else if self.current_cycle != signal.cycles.len() - 1
            && self.menu.action("move current cycle down")
        {
            signal
                .cycles
                .swap(self.current_cycle, self.current_cycle + 1);
            change_traffic_signal(signal, self.i, ui, ctx);
            self.current_cycle += 1;
        } else if signal.cycles.len() > 1 && self.menu.action("delete current cycle") {
            signal.cycles.remove(self.current_cycle);
            if self.current_cycle == signal.cycles.len() {
                self.current_cycle -= 1;
            }
            change_traffic_signal(signal, self.i, ui, ctx);
        } else if self.menu.action("add a new empty cycle") {
            signal
                .cycles
                .insert(self.current_cycle, Cycle::new(self.i, signal.cycles.len()));
            change_traffic_signal(signal, self.i, ui, ctx);
        } else if has_sidewalks && self.menu.action("add a new pedestrian scramble cycle") {
            let mut cycle = Cycle::new(self.i, signal.cycles.len());
            for t in ui.primary.map.get_turns_in_intersection(self.i) {
                if t.between_sidewalks() {
                    cycle.edit_turn(t, TurnPriority::Priority);
                }
            }
            signal.cycles.insert(self.current_cycle, cycle);
            change_traffic_signal(signal, self.i, ui, ctx);
        } else if has_sidewalks
            && self
                .menu
                .action("convert to dedicated pedestrian scramble cycle")
        {
            convert_to_ped_scramble(&mut signal, self.i, &ui.primary.map);
            change_traffic_signal(signal, self.i, ui, ctx);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details = Some(self.i);
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
        let cycle = &map.get_traffic_signal(self.i).cycles[self.current_cycle];
        for t in &ui.primary.draw_map.get_turns(self.i, map) {
            let arrow_color = match cycle.get_priority(t.id) {
                TurnPriority::Priority => ui
                    .cs
                    .get_def("priority turn in current cycle", Color::GREEN),
                TurnPriority::Yield => ui
                    .cs
                    .get_def("yield turn in current cycle", Color::rgb(255, 105, 180)),
                TurnPriority::Banned => ui.cs.get_def("turn not in current cycle", Color::BLACK),
                TurnPriority::Stop => panic!("Can't have TurnPriority::Stop in a traffic signal"),
            };
            t.draw_icon(
                &mut batch,
                &ctx.cs,
                arrow_color,
                self.icon_selected == Some(t.id),
            );
        }
        draw_signal_cycle(cycle, None, &mut batch, &ctx);
        if let Some(id) = self.icon_selected {
            DrawTurn::draw_dashed(
                map.get_t(id),
                &mut batch,
                ui.cs.get_def("selected turn", Color::RED),
            );
        }
        batch.draw(g);

        draw_signal_diagram(self.i, self.current_cycle, None, g, &ctx);

        self.menu.draw(g);
        if let Some(t) = self.icon_selected {
            CommonState::draw_osd(g, ui, Some(ID::Turn(t)));
        } else {
            CommonState::draw_osd(g, ui, None);
        }
    }
}

fn choose_preset(
    map: &Map,
    id: IntersectionID,
    mut wizard: WrappedWizard,
) -> Option<ControlTrafficSignal> {
    // TODO I wanted to do all of this work just once per wizard, but we can't touch map inside a
    // closure. Grr.
    let choices: Vec<(Option<MultiKey>, String, ControlTrafficSignal)> =
        ControlTrafficSignal::get_possible_policies(map, id)
            .into_iter()
            .map(|(name, ts)| (None, name, ts))
            .collect();

    wizard
        .choose_something::<ControlTrafficSignal>(
            "Use which preset for this intersection?",
            Box::new(move || choices.clone()),
        )
        .map(|(_, ts)| ts)
}

fn convert_to_ped_scramble(signal: &mut ControlTrafficSignal, i: IntersectionID, map: &Map) {
    // Remove Crosswalk turns from existing cycles.
    for cycle in signal.cycles.iter_mut() {
        // Crosswalks are usually only priority_turns, but also clear out from yield_turns.
        for t in map.get_turns_in_intersection(i) {
            if t.turn_type == TurnType::Crosswalk {
                cycle.priority_turns.remove(&t.id);
                cycle.yield_turns.remove(&t.id);
            }
        }

        // Blindly try to promote yield turns to protected, now that crosswalks are gone.
        let mut promoted = Vec::new();
        for t in &cycle.yield_turns {
            if cycle.could_be_priority_turn(*t, map) {
                cycle.priority_turns.insert(*t);
                promoted.push(*t);
            }
        }
        for t in promoted {
            cycle.yield_turns.remove(&t);
        }
    }

    let mut cycle = Cycle::new(i, signal.cycles.len());
    for t in map.get_turns_in_intersection(i) {
        if t.between_sidewalks() {
            cycle.edit_turn(t, TurnPriority::Priority);
        }
    }
    signal.cycles.push(cycle);
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

struct ChangeCycleDuration {
    cycle: Cycle,
    wizard: Wizard,
}

impl State for ChangeCycleDuration {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        if let Some(new_duration) = self.wizard.wrap(ctx).input_usize_prefilled(
            "How long should this cycle be?",
            format!("{}", self.cycle.duration.inner_seconds() as usize),
        ) {
            return Transition::PopWithData(Box::new(move |state, ui, ctx| {
                let editor = state.downcast_ref::<TrafficSignalEditor>().unwrap();
                let mut signal = ui.primary.map.get_traffic_signal(editor.i).clone();
                signal.cycles[editor.current_cycle].duration =
                    Duration::seconds(new_duration as f64);
                change_traffic_signal(signal, editor.i, ui, ctx);
            }));
        }
        if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

struct ChangePreset {
    i: IntersectionID,
    wizard: Wizard,
}

impl State for ChangePreset {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some(new_signal) = choose_preset(&ui.primary.map, self.i, self.wizard.wrap(ctx)) {
            return Transition::PopWithData(Box::new(move |state, ui, ctx| {
                let mut editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                editor.current_cycle = 0;
                change_traffic_signal(new_signal, editor.i, ui, ctx);
            }));
        }
        if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}
