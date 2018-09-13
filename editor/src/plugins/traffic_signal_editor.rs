// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO how to edit cycle time?

use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::UserInput;
use graphics::types::Color;
use map_model::Map;
use map_model::{IntersectionID, Turn};
use piston::input::Key;
use plugins::selection::{SelectionState, ID};

pub enum TrafficSignalEditor {
    Inactive,
    Active {
        i: IntersectionID,
        current_cycle: usize,
    },
}

impl TrafficSignalEditor {
    pub fn new() -> TrafficSignalEditor {
        TrafficSignalEditor::Inactive
    }

    pub fn start(i: IntersectionID) -> TrafficSignalEditor {
        TrafficSignalEditor::Active {
            i,
            current_cycle: 0,
        }
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        control_map: &mut ControlMap,
        current_selection: &SelectionState,
    ) -> bool {
        let mut new_state: Option<TrafficSignalEditor> = None;
        match self {
            TrafficSignalEditor::Inactive => {}
            TrafficSignalEditor::Active { i, current_cycle } => {
                if input.key_pressed(Key::Return, "quit the editor") {
                    new_state = Some(TrafficSignalEditor::Inactive);
                } else {
                    // Change cycles
                    {
                        let cycles = &control_map.traffic_signals[&i].cycles;
                        if let Some(n) = input.number_chosen(
                            cycles.len(),
                            &format!(
                                "Showing cycle {} of {}. Switch by pressing 1 - {}.",
                                *current_cycle + 1,
                                cycles.len(),
                                cycles.len()
                            ),
                        ) {
                            *current_cycle = n - 1;
                        }
                    }

                    // Change turns
                    if let SelectionState::Selected(ID::Turn(id)) = *current_selection {
                        if map.get_t(id).parent == *i {
                            let cycle =
                                &mut control_map.traffic_signals.get_mut(&i).unwrap().cycles
                                    [*current_cycle];
                            if cycle.contains(id) {
                                if input
                                    .key_pressed(Key::Backspace, "remove this turn from this cycle")
                                {
                                    cycle.remove(id);
                                }
                            } else if !cycle.conflicts_with(id, map) {
                                if input.key_pressed(Key::Space, "add this turn to this cycle") {
                                    cycle.add(id);
                                }
                            }
                        }
                    }
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }

        match self {
            TrafficSignalEditor::Inactive => false,
            _ => true,
        }
    }

    pub fn color_t(
        &self,
        t: &Turn,
        map: &Map,
        control_map: &ControlMap,
        cs: &ColorScheme,
    ) -> Option<Color> {
        match self {
            TrafficSignalEditor::Inactive => None,
            TrafficSignalEditor::Active { i, current_cycle } => {
                if t.parent != *i {
                    return Some(cs.get(Colors::TurnIrrelevant));
                }

                let cycle = &control_map.traffic_signals[&i].cycles[*current_cycle];

                if cycle.contains(t.id) {
                    Some(cs.get(Colors::SignalEditorTurnInCurrentCycle))
                } else if !cycle.conflicts_with(t.id, map) {
                    Some(cs.get(Colors::SignalEditorTurnCompatibleWithCurrentCycle))
                } else {
                    Some(cs.get(Colors::SignalEditorTurnConflictsWithCurrentCycle))
                }
                // TODO maybe something to indicate unused in any cycle so far
            }
        }
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        match self {
            TrafficSignalEditor::Active {
                i,
                current_cycle: _,
            } => *i == id,
            TrafficSignalEditor::Inactive => false,
        }
    }
}
