use dimensioned::si;
use ezgui::{Color, GfxCtx, Text, Wizard, WrappedWizard};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{ControlTrafficSignal, Cycle, IntersectionID, Map, TurnID, TurnPriority, TurnType};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::{draw_signal_cycle, DrawTurn};
use std::collections::HashSet;

pub enum TrafficSignalEditor {
    Inactive,
    // TODO Warn if there are empty cycles or if some turn is completely absent from the signal.
    Active {
        i: IntersectionID,
        current_cycle: usize,
        // The Wizard states are nested under here to remember things like current_cycle and keep
        // drawing stuff. Better way to represent nested states?
        cycle_duration_wizard: Option<Wizard>,
        preset_wizard: Option<Wizard>,
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
                            preset_wizard: None,
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
                ref mut preset_wizard,
                ref mut icon_selected,
            } => {
                ctx.hints.suppress_intersection_icon = Some(*i);
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
                } else if preset_wizard.is_some() {
                    if let Some(new_signal) = choose_preset(
                        &ctx.primary.map,
                        *i,
                        preset_wizard.as_mut().unwrap().wrap(input),
                    ) {
                        ctx.primary.map.edit_traffic_signal(new_signal);
                        *preset_wizard = None;
                    } else if preset_wizard.as_ref().unwrap().aborted() {
                        *preset_wizard = None;
                    }
                } else if let Some(ID::Turn(id)) = selected {
                    // We know this turn belongs to the current intersection, because we're only
                    // showing icons for this one.
                    *icon_selected = Some(id);

                    let mut signal = ctx.primary.map.get_traffic_signal(*i).clone();
                    {
                        let cycle = &mut signal.cycles[*current_cycle];
                        // Just one key to toggle between the 3 states
                        let next_priority = match cycle.get_priority(id) {
                            TurnPriority::Banned => {
                                if ctx.primary.map.get_t(id).turn_type == TurnType::Crosswalk {
                                    if cycle.could_be_priority_turn(id, &ctx.primary.map) {
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
                                if cycle.could_be_priority_turn(id, &ctx.primary.map) {
                                    Some(TurnPriority::Priority)
                                } else {
                                    Some(TurnPriority::Banned)
                                }
                            }
                            TurnPriority::Priority => Some(TurnPriority::Banned),
                        };
                        if let Some(pri) = next_priority {
                            if input.key_pressed(Key::Space, &format!("toggle to {:?}", pri)) {
                                cycle.edit_turn(id, pri, &ctx.primary.map);
                            }
                        }
                    }

                    ctx.primary.map.edit_traffic_signal(signal);
                } else {
                    *icon_selected = None;
                    if input.key_pressed(Key::Return, "quit the editor") {
                        new_state = Some(TrafficSignalEditor::Inactive);
                    }

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
                    } else if input.key_pressed(Key::P, "choose a preset for this intersection") {
                        *preset_wizard = Some(Wizard::new());
                    }

                    let mut signal = ctx.primary.map.get_traffic_signal(*i).clone();
                    if *current_cycle != 0 && input.key_pressed(Key::K, "move current cycle up") {
                        signal.cycles.swap(*current_cycle, *current_cycle - 1);
                        *current_cycle -= 1;
                        ctx.primary.map.edit_traffic_signal(signal);
                    } else if *current_cycle != signal.cycles.len() - 1
                        && input.key_pressed(Key::J, "move current cycle down")
                    {
                        signal.cycles.swap(*current_cycle, *current_cycle + 1);
                        *current_cycle += 1;
                        ctx.primary.map.edit_traffic_signal(signal);
                    } else if signal.cycles.len() > 1
                        && input.key_pressed(Key::Backspace, "delete current cycle")
                    {
                        signal.cycles.remove(*current_cycle);
                        if *current_cycle == signal.cycles.len() {
                            *current_cycle -= 1;
                        }
                        ctx.primary.map.edit_traffic_signal(signal);
                    } else if input.key_pressed(Key::N, "add a new empty cycle") {
                        signal.cycles.insert(*current_cycle, Cycle::new(*i));
                        ctx.primary.map.edit_traffic_signal(signal);
                    } else if input.key_pressed(Key::M, "add a new pedestrian scramble cycle") {
                        let mut cycle = Cycle::new(*i);
                        for t in ctx.primary.map.get_turns_in_intersection(*i) {
                            // edit_turn adds the other_crosswalk_id and asserts no duplicates.
                            if t.turn_type == TurnType::SharedSidewalkCorner
                                || (t.turn_type == TurnType::Crosswalk && t.id.src < t.id.dst)
                            {
                                cycle.edit_turn(t.id, TurnPriority::Priority, &ctx.primary.map);
                            }
                        }
                        signal.cycles.insert(*current_cycle, cycle);
                        ctx.primary.map.edit_traffic_signal(signal);
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
            preset_wizard,
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
            } else if let Some(wizard) = preset_wizard {
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

                Some(match cycle.get_priority(t) {
                    TurnPriority::Priority => {
                        ctx.cs.get("priority turn in current cycle", Color::GREEN)
                    }
                    TurnPriority::Yield => ctx
                        .cs
                        .get("yield turn in current cycle", Color::rgb(255, 105, 180)),
                    TurnPriority::Banned => ctx.cs.get("turn not in current cycle", Color::BLACK),
                    TurnPriority::Stop => {
                        panic!("Can't have TurnPriority::Stop in a traffic signal")
                    }
                })
            }
            _ => None,
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
    let mut choices: Vec<(String, ControlTrafficSignal)> = Vec::new();
    if let Some(ts) = ControlTrafficSignal::four_way_four_phase(map, id) {
        choices.push(("4-phase".to_string(), ts));
    }
    if let Some(ts) = ControlTrafficSignal::four_way_two_phase(map, id) {
        choices.push(("2-phase".to_string(), ts));
    }
    if let Some(ts) = ControlTrafficSignal::three_way(map, id) {
        choices.push(("2-phase".to_string(), ts));
    }
    if let Some(ts) = ControlTrafficSignal::greedy_assignment(map, id) {
        choices.push(("arbitrary assignment".to_string(), ts));
    }

    wizard
        .choose_something::<ControlTrafficSignal>(
            "Use which preset for this intersection?",
            Box::new(move || choices.clone()),
        ).map(|(_, ts)| ts)
}
