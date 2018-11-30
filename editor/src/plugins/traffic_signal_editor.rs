// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO how to edit cycle time?

use control::TurnPriority;
use ezgui::Color;
use map_model::IntersectionID;
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

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

impl Plugin for TrafficSignalEditor {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let input = ctx.input;
        let map = &ctx.primary.map;
        let control_map = &mut ctx.primary.control_map;
        let selected = ctx.primary.current_selection;

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
                        if id.parent == *i {
                            let cycle =
                                &mut control_map.traffic_signals.get_mut(&i).unwrap().cycles
                                    [*current_cycle];
                            if cycle.get_priority(id) == TurnPriority::Priority {
                                if input
                                    .key_pressed(Key::Backspace, "remove this turn from this cycle")
                                {
                                    cycle.remove(id);
                                }
                            } else if cycle.could_be_priority_turn(id, map) {
                                if input.key_pressed(
                                    Key::Space,
                                    "add this turn to this cycle as priority",
                                ) {
                                    cycle.add(id, TurnPriority::Priority);
                                }
                            } else if cycle.get_priority(id) == TurnPriority::Stop {
                                if input.key_pressed(Key::Y, "add this turn to this cycle as yield")
                                {
                                    cycle.add(id, TurnPriority::Yield);
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

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (TrafficSignalEditor::Active { i, current_cycle }, ID::Turn(t)) => {
                if t.parent != *i {
                    return Some(ctx.cs.get("irrelevant turn", Color::grey(0.3)));
                }

                let cycle = &ctx.control_map.traffic_signals[&i].cycles[*current_cycle];

                // TODO maybe something to indicate unused in any cycle so far
                let could_be_priority = cycle.could_be_priority_turn(t, ctx.map);
                match cycle.get_priority(t) {
                    TurnPriority::Priority => {
                        Some(ctx.cs.get("priority turn in current cycle", Color::GREEN))
                    }
                    TurnPriority::Yield => if could_be_priority {
                        Some(ctx.cs.get(
                            "yield turn that could be priority turn",
                            Color::rgb(154, 205, 50),
                        ))
                    } else {
                        Some(ctx.cs.get("yield turn in current cycle", Color::YELLOW))
                    },
                    TurnPriority::Stop => if could_be_priority {
                        Some(
                            ctx.cs
                                .get("stop turn that could be priority", Color::rgb(103, 49, 71)),
                        )
                    } else {
                        Some(ctx.cs.get("turn conflicts with current cycle", Color::RED))
                    },
                }
            }
            _ => None,
        }
    }
}
