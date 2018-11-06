// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::{Color, GfxCtx};
use map_model::{IntersectionID, LaneID};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

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
                            ctx.draw_map.get_t(turn.id).draw_full(
                                g,
                                ctx.cs.get("all turns from one lane", Color::RED.alpha(0.5)),
                            );
                        },
                    }
                }
                //draw_map.get_l(id).draw_debug(g, cs, map.get_l(id));
            }
            TurnCyclerState::Intersection(id) => {
                if let Some(signal) = ctx.control_map.traffic_signals.get(&id) {
                    let (cycle, _) =
                        signal.current_cycle_and_remaining_time(ctx.sim.time.as_time());
                    for t in &cycle.turns {
                        ctx.draw_map.get_t(*t).draw_full(
                            g,
                            ctx.cs.get(
                                "turns allowed by traffic signal right now",
                                Color::GREEN.alpha(0.5),
                            ),
                        );
                    }
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
