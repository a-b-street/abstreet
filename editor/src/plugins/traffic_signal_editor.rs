// TODO how to edit cycle time?

use ezgui::GfxCtx;
use map_model::{IntersectionID, TurnPriority};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::draw_signal_cycle;

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
}

impl Plugin for TrafficSignalEditor {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let input = ctx.input;
        let map = &mut ctx.primary.map;
        let selected = ctx.primary.current_selection;

        if *self == TrafficSignalEditor::Inactive {
            match selected {
                Some(ID::Intersection(id)) => {
                    if map.maybe_get_traffic_signal(id).is_some()
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
                        let cycles = &map.get_traffic_signal(*i).cycles;
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
                            let mut signal = map.get_traffic_signal(*i).clone();
                            {
                                let cycle = &mut signal.cycles[*current_cycle];
                                if cycle.get_priority(id) == TurnPriority::Priority {
                                    if input.key_pressed(
                                        Key::Backspace,
                                        "remove this turn from this cycle",
                                    ) {
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
                                    if input
                                        .key_pressed(Key::Y, "add this turn to this cycle as yield")
                                    {
                                        cycle.add(id, TurnPriority::Yield);
                                    }
                                }
                            }

                            map.edit_traffic_signal(signal);
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

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let TrafficSignalEditor::Active { i, current_cycle } = self {
            draw_signal_cycle(
                &ctx.map.get_traffic_signal(*i).cycles[*current_cycle],
                *i,
                g,
                ctx,
            );
        }
    }
}
