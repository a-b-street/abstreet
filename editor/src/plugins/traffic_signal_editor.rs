use ezgui::{Color, GfxCtx, Text};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{IntersectionID, TurnPriority};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::draw_signal_cycle;
use std::collections::HashSet;

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
        let selected = ctx.primary.current_selection;

        if *self == TrafficSignalEditor::Inactive {
            match selected {
                Some(ID::Intersection(id)) => {
                    if ctx.primary.map.maybe_get_traffic_signal(id).is_some()
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
                    ctx.hints.suppress_traffic_signal_icon = Some(*i);
                    ctx.hints.hide_crosswalks.extend(
                        ctx.primary.map.get_traffic_signal(*i).cycles[*current_cycle]
                            .get_absent_crosswalks(ctx.primary.map.get_turns_in_intersection(*i)),
                    );

                    // Change cycles
                    {
                        let cycles = &ctx.primary.map.get_traffic_signal(*i).cycles;
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
                            let mut signal = ctx.primary.map.get_traffic_signal(*i).clone();
                            {
                                let cycle = &mut signal.cycles[*current_cycle];
                                if cycle.get_priority(id) == TurnPriority::Priority {
                                    if input.key_pressed(
                                        Key::Backspace,
                                        "remove this turn from this cycle",
                                    ) {
                                        cycle.remove(id);
                                    }
                                } else if cycle.could_be_priority_turn(id, &ctx.primary.map) {
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

                            ctx.primary.map.edit_traffic_signal(signal);
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
            let cycles = &ctx.map.get_traffic_signal(*i).cycles;

            draw_signal_cycle(
                &cycles[*current_cycle],
                *i,
                g,
                ctx.cs,
                ctx.map,
                ctx.draw_map,
                &ctx.hints.hide_crosswalks,
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

            let old_ctx = g.fork_screenspace();
            g.draw_polygon(
                ctx.cs.get("signal editor panel", Color::BLACK.alpha(0.95)),
                &Polygon::rectangle_topleft(
                    Pt2D::new(10.0, 10.0),
                    2.0 * width * zoom,
                    (padding + height) * (cycles.len() as f64) * zoom,
                ),
            );
            // TODO Padding and offsets all a bit off. Abstractions are a bit awkward. Want to
            // center a map-space thing inside a screen-space box.
            g.draw_polygon(
                ctx.cs.get(
                    "current cycle in signal editor panel",
                    Color::BLUE.alpha(0.95),
                ),
                &Polygon::rectangle_topleft(
                    Pt2D::new(
                        10.0,
                        10.0 + (padding + height) * (*current_cycle as f64) * zoom,
                    ),
                    2.0 * width * zoom,
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
                let mut hide_crosswalks = HashSet::new();
                hide_crosswalks
                    .extend(cycle.get_absent_crosswalks(ctx.map.get_turns_in_intersection(*i)));
                draw_signal_cycle(
                    &cycle,
                    *i,
                    g,
                    ctx.cs,
                    ctx.map,
                    ctx.draw_map,
                    &hide_crosswalks,
                );

                let mut txt = Text::new();
                txt.add_line(format!("Cycle {}: {}", idx + 1, cycle.duration));
                ctx.canvas.draw_text_at_screenspace_topleft(
                    g,
                    txt,
                    (
                        10.0 + (width * zoom),
                        10.0 + (padding + height) * (idx as f64) * zoom,
                    ),
                );
            }

            g.unfork(old_ctx);
        }
    }
}
