use control::{ControlTrafficSignal, TurnPriority};
use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::Circle;
use map_model::{IntersectionID, LaneID, TurnType, LANE_THICKNESS};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::turn_markings;

#[derive(Clone, Debug)]
pub enum TurnCyclerState {
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
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, selected) = (ctx.input, ctx.primary.current_selection);

        let current_id = match selected {
            Some(ID::Lane(id)) => id,
            Some(ID::Intersection(id)) => {
                *self = TurnCyclerState::Intersection(id);
                return false;
            }
            _ => {
                *self = TurnCyclerState::Inactive;
                return false;
            }
        };

        let mut new_state: Option<TurnCyclerState> = None;
        match self {
            TurnCyclerState::Inactive | TurnCyclerState::Intersection(_) => {
                new_state = Some(TurnCyclerState::Active(current_id, None));
            }
            TurnCyclerState::Active(old_id, current_turn_index) => {
                if current_id != *old_id {
                    new_state = Some(TurnCyclerState::Inactive);
                } else if input.key_pressed(Key::Tab, "cycle through this lane's turns") {
                    let idx = match *current_turn_index {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    new_state = Some(TurnCyclerState::Active(current_id, Some(idx)));
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            TurnCyclerState::Inactive => false,
            // Only once they start tabbing through turns does this plugin block other input.
            TurnCyclerState::Active(_, current_turn_index) => current_turn_index.is_some(),
            TurnCyclerState::Intersection(_) => false,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::Active(l, current_turn_index) => {
                let relevant_turns = ctx.map.get_turns_from_lane(*l);
                if !relevant_turns.is_empty() {
                    match current_turn_index {
                        Some(idx) => {
                            let turn = relevant_turns[idx % relevant_turns.len()];
                            let draw_turn = ctx.draw_map.get_t(turn.id);
                            draw_turn.draw_full(g, ctx.cs.get("current selected turn", Color::RED));
                        }
                        None => for turn in &relevant_turns {
                            let color = match turn.turn_type {
                                TurnType::SharedSidewalkCorner => {
                                    ctx.cs.get("shared sidewalk corner turn", Color::BLACK)
                                }
                                TurnType::Crosswalk => ctx.cs.get("crosswalk turn", Color::WHITE),
                                TurnType::Other => ctx.cs.get("other kind of type", Color::RED),
                            }.alpha(0.5);
                            ctx.draw_map.get_t(turn.id).draw_full(g, color);
                        },
                    }
                }
                //draw_map.get_l(id).draw_debug(g, cs, map.get_l(id));
            }
            TurnCyclerState::Intersection(id) => {
                if let Some(signal) = ctx.control_map.traffic_signals.get(&id) {
                    draw_traffic_signal(signal, g, ctx);
                }
            }
        }
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
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

fn draw_traffic_signal(signal: &ControlTrafficSignal, g: &mut GfxCtx, ctx: Ctx) {
    // TODO Cycle might be over-run; should depict that by asking sim layer.
    // TODO It'd be cool to indicate remaining time in the cycle by slowly dimming the color or
    // something.
    let (cycle, _) = signal.current_cycle_and_remaining_time(ctx.sim.time.as_time());

    // First style: draw green turn arrows on the lanes. Hard to see.
    if false {
        let color = Some(
            ctx.cs
                .get("turns allowed by traffic signal right now", Color::GREEN),
        );
        // TODO Maybe don't show SharedSidewalkCorners at all, and highlight the
        // Crosswalks.
        for t in &cycle.priority_turns {
            for m in turn_markings(ctx.map.get_t(*t), ctx.map) {
                m.draw(g, ctx.cs, color);
            }
        }
    }

    // Second style: draw little circles on the incoming lanes to indicate what turns are possible.
    {
        for l in &ctx.map.get_i(signal.id).incoming_lanes {
            let mut num_green = 0;
            let mut num_yield = 0;
            let mut num_red = 0;
            for (t, _) in ctx.map.get_next_turns_and_lanes(*l, signal.id) {
                match cycle.get_priority(t.id) {
                    TurnPriority::Priority => {
                        num_green += 1;
                    }
                    TurnPriority::Yield => {
                        num_yield += 1;
                    }
                    TurnPriority::Stop => {
                        num_red += 1;
                    }
                };
            }

            // TODO Adjust this more.
            if num_green == 0 && num_yield == 0 {
                continue;
            }

            let color = if num_yield == 0 && num_red == 0 {
                ctx.cs.get(
                    "all turns from lane allowed by traffic signal right now",
                    Color::GREEN,
                )
            } else {
                // TODO Flashing green? :P
                ctx.cs.get(
                    "some turns from lane allowed by traffic signal right now",
                    Color::YELLOW,
                )
            };

            let lane = ctx.map.get_l(*l);
            if let Some((pt, _)) = lane.safe_dist_along(lane.length() - (LANE_THICKNESS * si::M)) {
                g.draw_circle(color, &Circle::new(pt, 1.0));
            }
        }
    }
}
