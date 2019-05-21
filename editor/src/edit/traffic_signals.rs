use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::GameState;
use crate::helpers::ID;
use crate::render::{draw_signal_cycle, draw_signal_diagram, DrawCtx, DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{Color, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu, Wizard, WrappedWizard};
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
                (Some(Key::Escape), "quit"),
                (Some(Key::D), "change cycle duration"),
                (Some(Key::P), "choose a preset signal"),
                (Some(Key::R), "reset to original"),
                (Some(Key::K), "move current cycle up"),
                (Some(Key::J), "move current cycle down"),
                (Some(Key::UpArrow), "select previous cycle"),
                (Some(Key::DownArrow), "select next cycle"),
                (Some(Key::Backspace), "delete current cycle"),
                (Some(Key::N), "add a new empty cycle"),
                (Some(Key::M), "add a new pedestrian scramble cycle"),
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

    // Returns true if the editor is done and we should go back to main edit mode.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.icon_selected = None;
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
                .wrap(ctx.input, ctx.canvas)
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
                self.preset_wizard
                    .as_mut()
                    .unwrap()
                    .wrap(ctx.input, ctx.canvas),
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
                return true;
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
            apply_map_edits(ui, ctx, new_edits);
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, state: &GameState) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details = Some(self.i);
            state
                .ui
                .draw(g, opts, &state.ui.primary.sim, &ShowEverything::new());
        }

        let mut batch = GeomBatch::new();
        let ctx = DrawCtx {
            cs: &state.ui.cs,
            map: &state.ui.primary.map,
            draw_map: &state.ui.primary.draw_map,
            sim: &state.ui.primary.sim,
        };
        let map = &state.ui.primary.map;
        let cycle = &map.get_traffic_signal(self.i).cycles[self.current_cycle];
        for t in &state.ui.primary.draw_map.get_turns(self.i, map) {
            let arrow_color = match cycle.get_priority(t.id) {
                TurnPriority::Priority => state
                    .ui
                    .cs
                    .get_def("priority turn in current cycle", Color::GREEN),
                TurnPriority::Yield => state
                    .ui
                    .cs
                    .get_def("yield turn in current cycle", Color::rgb(255, 105, 180)),
                TurnPriority::Banned => state
                    .ui
                    .cs
                    .get_def("turn not in current cycle", Color::BLACK),
                TurnPriority::Stop => panic!("Can't have TurnPriority::Stop in a traffic signal"),
            };
            t.draw_icon(
                &mut batch,
                &ctx.cs,
                arrow_color,
                self.icon_selected == Some(t.id),
            );
        }
        draw_signal_cycle(cycle, None, g, &ctx);
        if let Some(id) = self.icon_selected {
            DrawTurn::draw_dashed(map.get_t(id), &mut batch, state.ui.cs.get("selected turn"));
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
            CommonState::draw_osd(g, &state.ui, Some(ID::Turn(t)));
        } else {
            CommonState::draw_osd(g, &state.ui, None);
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
    let choices: Vec<(Option<Key>, String, ControlTrafficSignal)> =
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
