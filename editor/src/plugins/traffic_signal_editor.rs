// TODO how to edit cycle time?

use ezgui::{Color, GfxCtx};
use geom::{Bounds, Polygon, Pt2D};
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

    fn draw(&self, g: &mut GfxCtx, mut ctx: Ctx) {
        if let TrafficSignalEditor::Active { i, current_cycle } = self {
            let cycles = &ctx.map.get_traffic_signal(*i).cycles;

            draw_signal_cycle(
                &cycles[*current_cycle],
                *i,
                if ctx.current_selection == Some(ID::Intersection(*i)) {
                    ctx.cs.get("selected", Color::BLUE)
                } else {
                    ctx.cs.get("unchanged intersection", Color::grey(0.6))
                },
                g,
                &mut ctx,
            );

            // Draw all of the cycles off to the side
            let padding = 5.0;
            let zoom = 10.0;
            let (top_left, width, height) = {
                let mut b = Bounds::new();
                for pt in &ctx.map.get_i(*i).polygon {
                    b.update(*pt);
                }
                (
                    Pt2D::new(b.min_x, b.min_y),
                    b.max_x - b.min_x,
                    // Vertically pad
                    b.max_y - b.min_y,
                )
            };

            let panel_bg_color = ctx.cs.get("signal editor panel", Color::BLACK.alpha(0.95));
            let panel_selected_color = ctx.cs.get(
                "current cycle in signal editor panel",
                Color::BLUE.alpha(0.95),
            );
            let old_ctx = g.fork_screenspace();
            g.draw_polygon(
                panel_bg_color,
                &Polygon::rectangle_topleft(
                    Pt2D::new(10.0, 10.0),
                    width * zoom,
                    (padding + height) * (cycles.len() as f64) * zoom,
                ),
            );
            // TODO Padding and offsets all a bit off. Abstractions are a bit awkward. Want to
            // center a map-space thing inside a screen-space box.
            g.draw_polygon(
                panel_selected_color,
                &Polygon::rectangle_topleft(
                    Pt2D::new(
                        10.0,
                        10.0 + (padding + height) * (*current_cycle as f64) * zoom,
                    ),
                    width * zoom,
                    (padding + height) * zoom,
                ),
            );

            for (idx, cycle) in cycles.iter().enumerate() {
                g.fork(
                    // TODO Apply the offset here too?
                    Pt2D::new(
                        top_left.x(),
                        top_left.y() - height * (idx as f64) - padding * ((idx as f64) + 1.0),
                    ),
                    zoom,
                );
                draw_signal_cycle(
                    cycle,
                    *i,
                    if idx == *current_cycle {
                        panel_selected_color
                    } else {
                        panel_bg_color
                    },
                    g,
                    &mut ctx,
                );
            }

            g.unfork(old_ctx);
        }
    }
}
