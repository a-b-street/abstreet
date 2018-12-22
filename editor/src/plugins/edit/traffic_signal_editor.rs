use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{draw_signal_cycle, DrawTurn};
use dimensioned::si;
use ezgui::{Color, GfxCtx, Key, ScreenPt, Text, Wizard, WrappedWizard};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{ControlTrafficSignal, Cycle, IntersectionID, Map, TurnID, TurnPriority, TurnType};

// TODO Warn if there are empty cycles or if some turn is completely absent from the signal.
pub struct TrafficSignalEditor {
    i: IntersectionID,
    current_cycle: usize,
    // The Wizard states are nested under here to remember things like current_cycle and keep
    // drawing stuff. Better way to represent nested states?
    cycle_duration_wizard: Option<Wizard>,
    preset_wizard: Option<Wizard>,
    icon_selected: Option<TurnID>,
}

impl TrafficSignalEditor {
    pub fn new(ctx: &mut PluginCtx) -> Option<TrafficSignalEditor> {
        if let Some(ID::Intersection(id)) = ctx.primary.current_selection {
            if ctx.primary.map.maybe_get_traffic_signal(id).is_some()
                && ctx
                    .input
                    .contextual_action(Key::E, &format!("edit traffic signal for {}", id))
            {
                return Some(TrafficSignalEditor {
                    i: id,
                    current_cycle: 0,
                    cycle_duration_wizard: None,
                    preset_wizard: None,
                    icon_selected: None,
                });
            }
        }
        None
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        self.i == id
    }
}

impl Plugin for TrafficSignalEditor {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let input = &mut ctx.input;
        let selected = ctx.primary.current_selection;

        input.set_mode_with_prompt(
            "Traffic Signal Editor",
            format!("Traffic Signal Editor for {}", self.i),
            &ctx.canvas,
        );

        ctx.hints.suppress_traffic_signal_details = Some(self.i);
        for t in ctx.primary.map.get_turns_in_intersection(self.i) {
            // TODO bit weird, now looks like there's missing space between some icons. Do
            // we ever need to have an icon for SharedSidewalkCorner?
            if t.turn_type == TurnType::SharedSidewalkCorner {
                ctx.hints.hide_turn_icons.insert(t.id);
            }
        }

        if self.cycle_duration_wizard.is_some() {
            if let Some(new_duration) = self
                .cycle_duration_wizard
                .as_mut()
                .unwrap()
                .wrap(input, ctx.canvas)
                .input_usize_prefilled(
                    "How long should this cycle be?",
                    format!(
                        "{}",
                        ctx.primary.map.get_traffic_signal(self.i).cycles[self.current_cycle]
                            .duration
                            .value_unsafe as usize
                    ),
                )
            {
                let mut signal = ctx.primary.map.get_traffic_signal(self.i).clone();
                signal.cycles[self.current_cycle].edit_duration((new_duration as f64) * si::S);
                ctx.primary.map.edit_traffic_signal(signal);
                self.cycle_duration_wizard = None;
            } else if self.cycle_duration_wizard.as_ref().unwrap().aborted() {
                self.cycle_duration_wizard = None;
            }
        } else if self.preset_wizard.is_some() {
            if let Some(new_signal) = choose_preset(
                &ctx.primary.map,
                self.i,
                self.preset_wizard.as_mut().unwrap().wrap(input, ctx.canvas),
            ) {
                ctx.primary.map.edit_traffic_signal(new_signal);
                self.preset_wizard = None;
            } else if self.preset_wizard.as_ref().unwrap().aborted() {
                self.preset_wizard = None;
            }
        } else if let Some(ID::Turn(id)) = selected {
            // We know this turn belongs to the current intersection, because we're only
            // showing icons for this one.
            self.icon_selected = Some(id);

            let mut signal = ctx.primary.map.get_traffic_signal(self.i).clone();
            {
                let cycle = &mut signal.cycles[self.current_cycle];
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
                    if input.contextual_action(
                        Key::Space,
                        &format!("toggle from {:?} to {:?}", cycle.get_priority(id), pri),
                    ) {
                        cycle.edit_turn(id, pri, &ctx.primary.map);
                    }
                }
            }

            ctx.primary.map.edit_traffic_signal(signal);
        } else {
            self.icon_selected = None;
            if input.modal_action("quit") {
                return false;
            }

            if self.current_cycle != 0 && input.modal_action("select previous cycle") {
                self.current_cycle -= 1;
            }
            if self.current_cycle != ctx.primary.map.get_traffic_signal(self.i).cycles.len() - 1
                && input.modal_action("select next cycle")
            {
                self.current_cycle += 1;
            }

            if input.modal_action("change cycle duration") {
                self.cycle_duration_wizard = Some(Wizard::new());
            } else if input.modal_action("choose a preset signal") {
                self.preset_wizard = Some(Wizard::new());
            }

            let has_sidewalks = ctx
                .primary
                .map
                .get_turns_in_intersection(self.i)
                .iter()
                .any(|t| t.between_sidewalks());

            let mut signal = ctx.primary.map.get_traffic_signal(self.i).clone();
            if self.current_cycle != 0 && input.modal_action("move current cycle up") {
                signal
                    .cycles
                    .swap(self.current_cycle, self.current_cycle - 1);
                self.current_cycle -= 1;
                ctx.primary.map.edit_traffic_signal(signal);
            } else if self.current_cycle != signal.cycles.len() - 1
                && input.modal_action("move current cycle down")
            {
                signal
                    .cycles
                    .swap(self.current_cycle, self.current_cycle + 1);
                self.current_cycle += 1;
                ctx.primary.map.edit_traffic_signal(signal);
            } else if signal.cycles.len() > 1 && input.modal_action("delete current cycle") {
                signal.cycles.remove(self.current_cycle);
                if self.current_cycle == signal.cycles.len() {
                    self.current_cycle -= 1;
                }
                ctx.primary.map.edit_traffic_signal(signal);
            } else if input.modal_action("add a new empty cycle") {
                signal.cycles.insert(self.current_cycle, Cycle::new(self.i));
                ctx.primary.map.edit_traffic_signal(signal);
            } else if has_sidewalks && input.modal_action("add a new pedestrian scramble cycle") {
                let mut cycle = Cycle::new(self.i);
                for t in ctx.primary.map.get_turns_in_intersection(self.i) {
                    // edit_turn adds the other_crosswalk_id and asserts no duplicates.
                    if t.turn_type == TurnType::SharedSidewalkCorner
                        || (t.turn_type == TurnType::Crosswalk && t.id.src < t.id.dst)
                    {
                        cycle.edit_turn(t.id, TurnPriority::Priority, &ctx.primary.map);
                    }
                }
                signal.cycles.insert(self.current_cycle, cycle);
                ctx.primary.map.edit_traffic_signal(signal);
            }
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        let cycles = &ctx.map.get_traffic_signal(self.i).cycles;

        draw_signal_cycle(
            &cycles[self.current_cycle],
            g,
            ctx.cs,
            ctx.map,
            ctx.draw_map,
        );

        // Draw all of the cycles off to the side
        let padding = 5.0;
        let zoom = 10.0;
        let (top_left, width, height) = {
            let mut b = Bounds::new();
            for pt in &ctx.map.get_i(self.i).polygon {
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
            ctx.cs
                .get_def("signal editor panel", Color::BLACK.alpha(0.95)),
            &Polygon::rectangle_topleft(
                Pt2D::new(10.0, 10.0),
                2.0 * width * zoom,
                (padding + height) * (cycles.len() as f64) * zoom,
            ),
        );
        // TODO Padding and offsets all a bit off. Abstractions are a bit awkward. Want to
        // center a map-space thing inside a screen-space box.
        g.draw_polygon(
            ctx.cs.get_def(
                "current cycle in signal editor panel",
                Color::BLUE.alpha(0.95),
            ),
            &Polygon::rectangle_topleft(
                Pt2D::new(
                    10.0,
                    10.0 + (padding + height) * (self.current_cycle as f64) * zoom,
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
            draw_signal_cycle(&cycle, g, ctx.cs, ctx.map, ctx.draw_map);

            ctx.canvas.draw_text_at_screenspace_topleft(
                g,
                Text::from_line(format!("Cycle {}: {}", idx + 1, cycle.duration)),
                ScreenPt::new(
                    10.0 + (width * zoom),
                    10.0 + (padding + height) * (idx as f64) * zoom,
                ),
            );
        }

        g.unfork(old_ctx);

        if let Some(id) = self.icon_selected {
            // TODO What should we do for currently banned turns?
            if cycles[self.current_cycle].get_priority(id) == TurnPriority::Yield {
                DrawTurn::draw_dashed(ctx.map.get_t(id), g, ctx.cs.get("selected"));
            } else {
                DrawTurn::draw_full(ctx.map.get_t(id), g, ctx.cs.get("selected"));
            }
        }

        if let Some(ref wizard) = self.cycle_duration_wizard {
            wizard.draw(g, ctx.canvas);
        } else if let Some(ref wizard) = self.preset_wizard {
            wizard.draw(g, ctx.canvas);
        }
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        if let ID::Turn(t) = obj {
            if t.parent != self.i {
                return None;
            }
            let cycle = &ctx.map.get_traffic_signal(self.i).cycles[self.current_cycle];

            return Some(match cycle.get_priority(t) {
                TurnPriority::Priority => ctx
                    .cs
                    .get_def("priority turn in current cycle", Color::GREEN),
                TurnPriority::Yield => ctx
                    .cs
                    .get_def("yield turn in current cycle", Color::rgb(255, 105, 180)),
                TurnPriority::Banned => ctx.cs.get_def("turn not in current cycle", Color::BLACK),
                TurnPriority::Stop => panic!("Can't have TurnPriority::Stop in a traffic signal"),
            });
        }
        None
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
        )
        .map(|(_, ts)| ts)
}
