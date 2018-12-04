use dimensioned::si;
use ezgui::{Color, GfxCtx, Text, Wizard};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{IntersectionID, TurnID, TurnPriority, TurnType};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::{draw_signal_cycle, DrawTurn};
use std::collections::HashSet;

pub enum TrafficSignalEditor {
    Inactive,
    Active {
        i: IntersectionID,
        current_cycle: usize,
        cycle_duration_wizard: Option<Wizard>,
        icon_selected: Option<TurnID>,
    },
}

impl TrafficSignalEditor {
    pub fn new() -> TrafficSignalEditor {
        TrafficSignalEditor::Inactive
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        match self {
            TrafficSignalEditor::Active { i, .. } => *i == id,
            _ => false,
        }
    }
}

impl Plugin for TrafficSignalEditor {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let input = ctx.input;
        let selected = ctx.primary.current_selection;

        let inactive = match self {
            TrafficSignalEditor::Inactive => true,
            _ => false,
        };
        if inactive {
            match selected {
                Some(ID::Intersection(id)) => {
                    if ctx.primary.map.maybe_get_traffic_signal(id).is_some()
                        && input.key_pressed(Key::E, &format!("edit traffic signal for {}", id))
                    {
                        *self = TrafficSignalEditor::Active {
                            i: id,
                            current_cycle: 0,
                            cycle_duration_wizard: None,
                            icon_selected: None,
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
            TrafficSignalEditor::Active {
                i,
                current_cycle,
                ref mut cycle_duration_wizard,
                ref mut icon_selected,
            } => {
                ctx.hints.suppress_traffic_signal_icon = Some(*i);
                ctx.hints.hide_crosswalks.extend(
                    ctx.primary.map.get_traffic_signal(*i).cycles[*current_cycle]
                        .get_absent_crosswalks(ctx.primary.map.get_turns_in_intersection(*i)),
                );
                for t in ctx.primary.map.get_turns_in_intersection(*i) {
                    // TODO bit weird, now looks like there's missing space between some icons. Do
                    // we ever need to have an icon for SharedSidewalkCorner?
                    if t.turn_type == TurnType::SharedSidewalkCorner {
                        ctx.hints.hide_turn_icons.insert(t.id);
                    }
                }

                if cycle_duration_wizard.is_some() {
                    if let Some(new_duration) = cycle_duration_wizard
                        .as_mut()
                        .unwrap()
                        .wrap(input)
                        .input_usize_prefilled(
                            "How long should this cycle be?",
                            format!(
                                "{}",
                                ctx.primary.map.get_traffic_signal(*i).cycles[*current_cycle]
                                    .duration
                                    .value_unsafe as usize
                            ),
                        ) {
                        let mut signal = ctx.primary.map.get_traffic_signal(*i).clone();
                        signal.cycles[*current_cycle].edit_duration((new_duration as f64) * si::S);
                        ctx.primary.map.edit_traffic_signal(signal);
                        *cycle_duration_wizard = None;
                    } else if cycle_duration_wizard.as_ref().unwrap().aborted() {
                        *cycle_duration_wizard = None;
                    }
                } else if input.key_pressed(Key::Return, "quit the editor") {
                    new_state = Some(TrafficSignalEditor::Inactive);
                } else {
                    *icon_selected = match selected {
                        Some(ID::Turn(id)) => Some(id),
                        _ => None,
                    };

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

                    if input.key_pressed(Key::D, "change cycle duration") {
                        *cycle_duration_wizard = Some(Wizard::new());
                    }

                    // Change turns
                    if let Some(ID::Turn(id)) = selected {
                        if id.parent == *i {
                            let mut signal = ctx.primary.map.get_traffic_signal(*i).clone();
                            {
                                let cycle = &mut signal.cycles[*current_cycle];
                                if cycle.get_priority(id) != TurnPriority::Stop && input
                                    .key_pressed(Key::Backspace, "remove this turn from this cycle")
                                {
                                    cycle.edit_turn(id, TurnPriority::Stop, &ctx.primary.map);
                                } else if cycle.could_be_priority_turn(id, &ctx.primary.map)
                                    && input.key_pressed(
                                        Key::Space,
                                        "add this turn to this cycle as priority",
                                    ) {
                                    cycle.edit_turn(id, TurnPriority::Priority, &ctx.primary.map);
                                } else if cycle.could_be_yield_turn(id, &ctx.primary.map) && input
                                    .key_pressed(Key::Y, "add this turn to this cycle as yield")
                                {
                                    cycle.edit_turn(id, TurnPriority::Yield, &ctx.primary.map);
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
        if let TrafficSignalEditor::Active {
            i,
            current_cycle,
            cycle_duration_wizard,
            icon_selected,
        } = self
        {
            let cycles = &ctx.map.get_traffic_signal(*i).cycles;

            draw_signal_cycle(
                &cycles[*current_cycle],
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
                draw_signal_cycle(&cycle, g, ctx.cs, ctx.map, ctx.draw_map, &hide_crosswalks);

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

            if let Some(id) = icon_selected {
                DrawTurn::draw_full(
                    ctx.map.get_t(*id),
                    g,
                    ctx.cs.get("selected turn icon", Color::BLUE.alpha(0.5)),
                );
            }

            if let Some(wizard) = cycle_duration_wizard {
                wizard.draw(g, ctx.canvas);
            }
        }
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (
                TrafficSignalEditor::Active {
                    i, current_cycle, ..
                },
                ID::Turn(t),
            ) => {
                if t.parent != *i {
                    return None;
                }
                let cycle = &ctx.map.get_traffic_signal(*i).cycles[*current_cycle];

                let could_be_priority = cycle.could_be_priority_turn(t, ctx.map);
                match cycle.get_priority(t) {
                    TurnPriority::Priority => {
                        Some(ctx.cs.get("priority turn in current cycle", Color::GREEN))
                    }
                    TurnPriority::Yield => if could_be_priority {
                        Some(
                            ctx.cs
                                .get("yield turn that could be priority turn", Color::YELLOW),
                        )
                    } else {
                        Some(
                            ctx.cs
                                .get("yield turn in current cycle", Color::rgb(255, 105, 180)),
                        )
                    },
                    TurnPriority::Stop => if could_be_priority {
                        Some(ctx.cs.get("stop turn that could be priority", Color::RED))
                    } else {
                        Some(
                            ctx.cs
                                .get("turn conflicts with current cycle", Color::BLACK),
                        )
                    },
                }
            }
            _ => None,
        }
    }
}
