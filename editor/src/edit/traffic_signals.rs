use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::{draw_signal_cycle, draw_signal_diagram, DrawCtx, DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, Color, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, ModalMenu, MultiKey, Wizard,
    WrappedWizard,
};
use geom::Duration;
use map_model::{ControlTrafficSignal, Cycle, IntersectionID, Map, TurnID, TurnPriority, TurnType};

// TODO Warn if there are empty cycles or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    menu: ModalMenu,
    i: IntersectionID,
    current_cycle: usize,
    // The Wizard states are nested under here to remember things like current_cycle and keep
    // drawing stuff. Better way to represent nested states?
    cycle_duration_wizard: Option<Wizard>,
    preset_wizard: Option<Wizard>,
    icon_selected: Option<TurnID>,
}

impl TrafficSignalEditor {
    pub fn new(id: IntersectionID, ctx: &mut EventCtx, ui: &mut UI) -> TrafficSignalEditor {
        ui.primary.current_selection = None;
        let menu = ModalMenu::new(
            &format!("Traffic Signal Editor for {}", id),
            vec![
                (hotkey(Key::Escape), "quit"),
                (hotkey(Key::D), "change cycle duration"),
                (hotkey(Key::P), "choose a preset signal"),
                (hotkey(Key::R), "reset to original"),
                (hotkey(Key::K), "move current cycle up"),
                (hotkey(Key::J), "move current cycle down"),
                (hotkey(Key::UpArrow), "select previous cycle"),
                (hotkey(Key::DownArrow), "select next cycle"),
                (hotkey(Key::Backspace), "delete current cycle"),
                (hotkey(Key::N), "add a new empty cycle"),
                (hotkey(Key::M), "add a new pedestrian scramble cycle"),
                (
                    hotkey(Key::B),
                    "convert to dedicated pedestrian scramble cycle",
                ),
            ],
            ctx,
        );
        TrafficSignalEditor {
            menu,
            i: id,
            current_cycle: 0,
            cycle_duration_wizard: None,
            preset_wizard: None,
            icon_selected: None,
        }
    }
}

impl State for TrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
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
        let mut changed = false;

        if self.cycle_duration_wizard.is_some() {
            if let Some(new_duration) = self
                .cycle_duration_wizard
                .as_mut()
                .unwrap()
                .wrap(ctx)
                .input_usize_prefilled(
                    "How long should this cycle be?",
                    format!(
                        "{}",
                        signal.cycles[self.current_cycle].duration.inner_seconds() as usize
                    ),
                )
            {
                signal.cycles[self.current_cycle].duration = Duration::seconds(new_duration as f64);
                changed = true;
                self.cycle_duration_wizard = None;
            } else if self.cycle_duration_wizard.as_ref().unwrap().aborted() {
                self.cycle_duration_wizard = None;
            }
        } else if self.preset_wizard.is_some() {
            if let Some(new_signal) = choose_preset(
                &ui.primary.map,
                self.i,
                self.preset_wizard.as_mut().unwrap().wrap(ctx),
            ) {
                signal = new_signal;
                changed = true;
                self.current_cycle = 0;
                self.preset_wizard = None;
            } else if self.preset_wizard.as_ref().unwrap().aborted() {
                self.preset_wizard = None;
            }
        } else if let Some(id) = self.icon_selected {
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
                    changed = true;
                }
            }
        } else {
            if self.menu.action("quit") {
                return (Transition::Pop, EventLoopMode::InputOnly);
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
                self.cycle_duration_wizard = Some(Wizard::new());
            } else if self.menu.action("choose a preset signal") {
                self.preset_wizard = Some(Wizard::new());
            } else if self.menu.action("reset to original") {
                signal = ControlTrafficSignal::get_possible_policies(&ui.primary.map, self.i)
                    .remove(0)
                    .1;
                changed = true;
                self.current_cycle = 0;
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
                changed = true;
                self.current_cycle -= 1;
            } else if self.current_cycle != signal.cycles.len() - 1
                && self.menu.action("move current cycle down")
            {
                signal
                    .cycles
                    .swap(self.current_cycle, self.current_cycle + 1);
                changed = true;
                self.current_cycle += 1;
            } else if signal.cycles.len() > 1 && self.menu.action("delete current cycle") {
                signal.cycles.remove(self.current_cycle);
                changed = true;
                if self.current_cycle == signal.cycles.len() {
                    self.current_cycle -= 1;
                }
            } else if self.menu.action("add a new empty cycle") {
                signal
                    .cycles
                    .insert(self.current_cycle, Cycle::new(self.i, signal.cycles.len()));
                changed = true;
            } else if has_sidewalks && self.menu.action("add a new pedestrian scramble cycle") {
                let mut cycle = Cycle::new(self.i, signal.cycles.len());
                for t in ui.primary.map.get_turns_in_intersection(self.i) {
                    if t.between_sidewalks() {
                        cycle.edit_turn(t, TurnPriority::Priority);
                    }
                }
                signal.cycles.insert(self.current_cycle, cycle);
                changed = true;
            } else if has_sidewalks
                && self
                    .menu
                    .action("convert to dedicated pedestrian scramble cycle")
            {
                convert_to_ped_scramble(&mut signal, self.i, &ui.primary.map);
                changed = true;
            }
        }

        if changed {
            let orig = ControlTrafficSignal::new(&ui.primary.map, self.i, &mut Timer::throwaway());
            let mut new_edits = ui.primary.map.get_edits().clone();
            if orig == signal {
                new_edits.traffic_signal_overrides.remove(&self.i);
            } else {
                new_edits.traffic_signal_overrides.insert(self.i, signal);
            }
            apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
        }

        (Transition::Keep, EventLoopMode::InputOnly)
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
            DrawTurn::draw_dashed(map.get_t(id), &mut batch, ui.cs.get("selected turn"));
        }
        batch.draw(g);

        draw_signal_diagram(self.i, self.current_cycle, None, g, &ctx);

        if let Some(ref wizard) = self.cycle_duration_wizard {
            wizard.draw(g);
        } else if let Some(ref wizard) = self.preset_wizard {
            wizard.draw(g);
        }

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
