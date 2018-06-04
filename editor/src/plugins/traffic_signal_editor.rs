// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO how to edit cycle time?

extern crate map_model;

use control::ControlMap;
use ezgui::canvas;
use ezgui::input::UserInput;
use geom::GeomMap;
use graphics::types::Color;
use map_model::Map;
use map_model::{IntersectionID, Turn};
use piston::input::Key;
use plugins::selection::SelectionState;

pub struct TrafficSignalEditor {
    i: IntersectionID,
    current_cycle: usize,
}

impl TrafficSignalEditor {
    pub fn new(i: IntersectionID) -> TrafficSignalEditor {
        TrafficSignalEditor {
            i,
            current_cycle: 0,
        }
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        geom_map: &GeomMap,
        control_map: &mut ControlMap,
        current_selection: &SelectionState,
    ) -> bool {
        if input.key_pressed(Key::Return, "Press enter to quit the editor") {
            return true;
        }

        // Change cycles
        {
            let cycles = &control_map.traffic_signals[&self.i].cycles;
            if let Some(n) = input.number_chosen(
                cycles.len(),
                &format!(
                    "Showing cycle {} of {}. Switch by pressing 1 - {}.",
                    self.current_cycle + 1,
                    cycles.len(),
                    cycles.len()
                ),
            ) {
                self.current_cycle = n - 1;
            }
        }

        // Change turns
        if let SelectionState::SelectedTurn(id) = *current_selection {
            if map.get_t(id).parent == self.i {
                let cycle = &mut control_map.traffic_signals.get_mut(&self.i).unwrap().cycles
                    [self.current_cycle];
                if cycle.contains(id) {
                    if input.key_pressed(
                        Key::Backspace,
                        "Press Backspace to remove this turn from this cycle",
                    ) {
                        cycle.remove(id);
                    }
                } else if !cycle.conflicts_with(id, geom_map) {
                    if input.key_pressed(Key::Space, "Press Space to add this turn to this cycle") {
                        cycle.add(id);
                    }
                }
            }
        }

        false
    }

    pub fn color_t(&self, t: &Turn, geom_map: &GeomMap, control_map: &ControlMap) -> Option<Color> {
        if t.parent != self.i {
            return Some(canvas::DARK_GREY);
        }

        let cycle = &control_map.traffic_signals[&self.i].cycles[self.current_cycle];

        if cycle.contains(t.id) {
            Some(canvas::GREEN)
        } else if !cycle.conflicts_with(t.id, geom_map) {
            Some([0.0, 1.0, 0.0, 0.2])
        } else {
            Some(canvas::RED)
        }
        // TODO maybe something to indicate unused in any cycle so far
    }
}
