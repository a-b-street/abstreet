// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO how to edit cycle time?

use colors::Colors;
use control::ControlMap;
use ezgui::{Color, UserInput};
use map_model::{IntersectionID, Map};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::Colorizer;

#[derive(PartialEq)]
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

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        control_map: &mut ControlMap,
        selected: Option<ID>,
    ) -> bool {
        if *self == TrafficSignalEditor::Inactive {
            match selected {
                Some(ID::Intersection(id)) => {
                    if control_map.traffic_signals.contains_key(&id)
                        && input.key_pressed(Key::E, &format!("edit traffic signal for {}", id))
                    {
                        *self = TrafficSignalEditor::Active {
                            i: id,
                            current_cycle: 0,
                        };
                        return true;
                    }
                }
                _ => {}
            }
        }

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
                    if let Some(ID::Turn(id)) = selected {
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

impl Colorizer for TrafficSignalEditor {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (TrafficSignalEditor::Active { i, current_cycle }, ID::Turn(t)) => {
                if t.parent != *i {
                    return Some(ctx.cs.get(Colors::TurnIrrelevant));
                }

                let cycle = &ctx.control_map.traffic_signals[&i].cycles[*current_cycle];

                if cycle.contains(t) {
                    Some(ctx.cs.get(Colors::SignalEditorTurnInCurrentCycle))
                } else if !cycle.conflicts_with(t, ctx.map) {
                    Some(
                        ctx.cs
                            .get(Colors::SignalEditorTurnCompatibleWithCurrentCycle),
                    )
                } else {
                    Some(
                        ctx.cs
                            .get(Colors::SignalEditorTurnConflictsWithCurrentCycle),
                    )
                }
                // TODO maybe something to indicate unused in any cycle so far
            }
            _ => None,
        }
    }
}
