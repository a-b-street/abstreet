use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{draw_signal_cycle, draw_stop_sign, stop_sign_rendering_hints, DrawTurn};
use ezgui::{Color, GfxCtx};
use geom::{Polygon, Pt2D};
use map_model::{IntersectionID, LaneID, TurnType};
use piston::input::Key;

#[derive(Clone, Debug)]
pub enum TurnCyclerState {
    // TODO Can probably simplify this?
    Inactive,
    Active(LaneID, Option<usize>),
    Intersection(IntersectionID),
}

impl TurnCyclerState {
    pub fn new() -> TurnCyclerState {
        TurnCyclerState::Inactive
    }
}

impl Plugin for TurnCyclerState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        let current_id = match ctx.primary.current_selection {
            Some(ID::Lane(id)) => id,
            Some(ID::Intersection(id)) => {
                *self = TurnCyclerState::Intersection(id);

                if let Some(signal) = ctx.primary.map.maybe_get_traffic_signal(id) {
                    let (cycle, _) =
                        signal.current_cycle_and_remaining_time(ctx.primary.sim.time.as_time());
                    ctx.hints.suppress_intersection_icon = Some(id);
                    ctx.hints
                        .hide_crosswalks
                        .extend(cycle.get_absent_crosswalks(&ctx.primary.map));
                } else if let Some(sign) = ctx.primary.map.maybe_get_stop_sign(id) {
                    stop_sign_rendering_hints(&mut ctx.hints, sign, &ctx.primary.map, ctx.cs);
                }
                return;
            }
            _ => {
                *self = TurnCyclerState::Inactive;
                return;
            }
        };

        match self {
            TurnCyclerState::Inactive | TurnCyclerState::Intersection(_) => {
                *self = TurnCyclerState::Active(current_id, None);
            }
            TurnCyclerState::Active(old_id, current_turn_index) => {
                if current_id != *old_id {
                    *self = TurnCyclerState::Inactive;
                } else if ctx
                    .input
                    .key_pressed(Key::Tab, "cycle through this lane's turns")
                {
                    let idx = match *current_turn_index {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    *self = TurnCyclerState::Active(current_id, Some(idx));
                }
            }
        };
    }

    fn new_draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::Active(l, current_turn_index) => {
                let relevant_turns = ctx.map.get_turns_from_lane(*l);
                if !relevant_turns.is_empty() {
                    match current_turn_index {
                        Some(idx) => {
                            let turn = relevant_turns[idx % relevant_turns.len()];
                            DrawTurn::draw_full(
                                turn,
                                g,
                                ctx.cs.get("current selected turn", Color::RED),
                            );
                        }
                        None => {
                            for turn in &relevant_turns {
                                let color = match turn.turn_type {
                                    TurnType::SharedSidewalkCorner => {
                                        ctx.cs.get("shared sidewalk corner turn", Color::BLACK)
                                    }
                                    TurnType::Crosswalk => {
                                        ctx.cs.get("crosswalk turn", Color::WHITE)
                                    }
                                    TurnType::Straight => ctx.cs.get("straight turn", Color::BLUE),
                                    TurnType::Right => ctx.cs.get("right turn", Color::GREEN),
                                    TurnType::Left => ctx.cs.get("left turn", Color::RED),
                                }
                                .alpha(0.5);
                                DrawTurn::draw_full(turn, g, color);
                            }
                        }
                    }
                }
            }
            TurnCyclerState::Intersection(id) => {
                if let Some(signal) = ctx.map.maybe_get_traffic_signal(*id) {
                    // TODO Cycle might be over-run; should depict that by asking sim layer.
                    let (cycle, time_left) =
                        signal.current_cycle_and_remaining_time(ctx.sim.time.as_time());

                    draw_signal_cycle(
                        cycle,
                        g,
                        ctx.cs,
                        ctx.map,
                        ctx.draw_map,
                        &ctx.hints.hide_crosswalks,
                    );

                    // Draw a little timer box in the top-left corner of the screen.
                    {
                        let old_ctx = g.fork_screenspace();
                        let width = 50.0;
                        let height = 100.0;
                        g.draw_polygon(
                            ctx.cs.get("timer foreground", Color::RED),
                            &Polygon::rectangle_topleft(Pt2D::new(10.0, 10.0), width, height),
                        );
                        g.draw_polygon(
                            ctx.cs.get("timer background", Color::BLACK),
                            &Polygon::rectangle_topleft(
                                Pt2D::new(10.0, 10.0),
                                width,
                                (time_left / cycle.duration).value_unsafe * height,
                            ),
                        );
                        g.unfork(old_ctx);
                    }
                } else if let Some(sign) = ctx.map.maybe_get_stop_sign(*id) {
                    draw_stop_sign(sign, g, ctx.cs, ctx.map);
                }
            }
        }
    }

    fn new_color_for(&self, obj: ID, ctx: &mut Ctx) -> Option<Color> {
        match (self, obj) {
            (TurnCyclerState::Active(l, Some(idx)), ID::Turn(t)) => {
                // Quickly prune irrelevant lanes
                if t.src != *l && t.dst != *l {
                    return None;
                }

                let relevant_turns = ctx.map.get_turns_from_lane(*l);
                if relevant_turns[idx % relevant_turns.len()].conflicts_with(ctx.map.get_t(t)) {
                    Some(ctx.cs.get(
                        "turn conflicts with current turn",
                        Color::rgba(255, 0, 0, 0.5),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
