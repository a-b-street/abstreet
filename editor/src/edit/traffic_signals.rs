use crate::edit::apply_map_edits;
use crate::game::GameState;
use crate::helpers::ID;
use crate::render::{draw_signal_cycle, draw_signal_diagram, DrawCtx, DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu, ScreenPt, Wizard, WrappedWizard};
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

    diagram_top_left: ScreenPt,
}

impl TrafficSignalEditor {
    pub fn new(id: IntersectionID, ctx: &mut EventCtx) -> TrafficSignalEditor {
        let menu = ModalMenu::new(
            &format!("Traffic Signal Editor for {}", id),
            vec![
                (Key::Escape, "quit"),
                (Key::D, "change cycle duration"),
                (Key::P, "choose a preset signal"),
                (Key::K, "move current cycle up"),
                (Key::J, "move current cycle down"),
                (Key::UpArrow, "select previous cycle"),
                (Key::DownArrow, "select next cycle"),
                (Key::Backspace, "delete current cycle"),
                (Key::N, "add a new empty cycle"),
                (Key::M, "add a new pedestrian scramble cycle"),
            ],
            ctx,
        );
        let diagram_top_left = menu.get_bottom_left(ctx);
        TrafficSignalEditor {
            menu,
            i: id,
            current_cycle: 0,
            cycle_duration_wizard: None,
            preset_wizard: None,
            icon_selected: None,
            diagram_top_left,
        }
    }

    // Returns true if the editor is done and we should go back to main edit mode.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        self.menu.handle_event(ctx);
        self.diagram_top_left = self.menu.get_bottom_left(ctx);
        ctx.canvas.handle_event(ctx.input);

        ui.primary.current_selection = ui.handle_mouseover(
            ctx,
            Some(self.i),
            &ui.primary.sim,
            &ShowEverything::new(),
            false,
        );

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
                self.preset_wizard = None;
            } else if self.preset_wizard.as_ref().unwrap().aborted() {
                self.preset_wizard = None;
            }
        } else if let Some(ID::Turn(id)) = ui.primary.current_selection {
            // We know this turn belongs to the current intersection, because we're only
            // showing icons for this one.
            self.icon_selected = Some(id);

            {
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
                        cycle.edit_turn(id, pri);
                        changed = true;
                    }
                }
            }
        } else {
            self.icon_selected = None;
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
                    // edit_turn adds the other_crosswalk_id and asserts no duplicates.
                    if t.turn_type == TurnType::SharedSidewalkCorner
                        || (t.turn_type == TurnType::Crosswalk && t.id.src < t.id.dst)
                    {
                        cycle.edit_turn(t.id, TurnPriority::Priority);
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
        let cycle = &state.ui.primary.map.get_traffic_signal(self.i).cycles[self.current_cycle];
        let mut opts = DrawOptions::new();
        opts.show_turn_icons_for = Some(self.i);
        opts.suppress_traffic_signal_details = Some(self.i);
        for t in &state.ui.primary.map.get_i(self.i).turns {
            opts.override_colors.insert(
                ID::Turn(*t),
                match cycle.get_priority(*t) {
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
                    TurnPriority::Stop => {
                        panic!("Can't have TurnPriority::Stop in a traffic signal")
                    }
                },
            );
        }
        state
            .ui
            .draw(g, opts, &state.ui.primary.sim, &ShowEverything::new());

        self.menu.draw(g);

        let ctx = DrawCtx {
            cs: &state.ui.cs,
            map: &state.ui.primary.map,
            draw_map: &state.ui.primary.draw_map,
            sim: &state.ui.primary.sim,
        };
        draw_signal_cycle(cycle, g, &ctx);

        draw_signal_diagram(
            self.i,
            self.current_cycle,
            None,
            self.diagram_top_left.y,
            g,
            &ctx,
        );

        if let Some(id) = self.icon_selected {
            DrawTurn::draw_dashed(
                state.ui.primary.map.get_t(id),
                g,
                state.ui.cs.get("selected turn"),
            );
        }

        if let Some(ref wizard) = self.cycle_duration_wizard {
            wizard.draw(g);
        } else if let Some(ref wizard) = self.preset_wizard {
            wizard.draw(g);
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
    let mut choices: Vec<(Option<Key>, String, ControlTrafficSignal)> = Vec::new();
    if let Some(ts) = ControlTrafficSignal::four_way_four_phase(map, id) {
        choices.push((None, "four-phase".to_string(), ts));
    }
    if let Some(ts) = ControlTrafficSignal::four_way_two_phase(map, id) {
        choices.push((None, "two-phase".to_string(), ts));
    }
    if let Some(ts) = ControlTrafficSignal::three_way(map, id) {
        choices.push((None, "three-phase".to_string(), ts));
    }
    choices.push((
        None,
        "arbitrary assignment".to_string(),
        ControlTrafficSignal::greedy_assignment(map, id).unwrap(),
    ));

    wizard
        .choose_something::<ControlTrafficSignal>(
            "Use which preset for this intersection?",
            Box::new(move || choices.clone()),
        )
        .map(|(_, ts)| ts)
}
