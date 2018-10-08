// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::{Color, GfxCtx, UserInput};
use map_model::{IntersectionID, LaneID, Map};
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::Colorizer;
use render::DrawMap;
use sim::Tick;

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

    pub fn event(&mut self, input: &mut UserInput, selected: Option<ID>) -> bool {
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

    pub fn draw(
        &self,
        map: &Map,
        draw_map: &DrawMap,
        control_map: &ControlMap,
        time: Tick,
        cs: &ColorScheme,
        g: &mut GfxCtx,
    ) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::Active(l, current_turn_index) => {
                let relevant_turns = map.get_turns_from_lane(*l);
                if !relevant_turns.is_empty() {
                    match current_turn_index {
                        Some(idx) => {
                            let turn = relevant_turns[idx % relevant_turns.len()];
                            let draw_turn = draw_map.get_t(turn.id);
                            draw_turn.draw_full(g, cs.get(Colors::Turn));
                        }
                        None => for turn in &relevant_turns {
                            draw_map.get_t(turn.id).draw_full(g, cs.get(Colors::Turn));
                        },
                    }
                }
                //draw_map.get_l(id).draw_debug(g, cs, map.get_l(id));
            }
            TurnCyclerState::Intersection(id) => {
                if let Some(signal) = control_map.traffic_signals.get(&id) {
                    let (cycle, _) = signal.current_cycle_and_remaining_time(time.as_time());
                    for t in &cycle.turns {
                        draw_map.get_t(*t).draw_full(g, cs.get(Colors::Turn));
                    }
                }
            }
        }
    }
}

impl Colorizer for TurnCyclerState {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (TurnCyclerState::Active(l, Some(idx)), ID::Turn(t)) => {
                // TODO quickly prune if t doesnt go from or to l

                let relevant_turns = ctx.map.get_turns_from_lane(*l);
                if relevant_turns[idx % relevant_turns.len()].conflicts_with(ctx.map.get_t(t)) {
                    Some(ctx.cs.get(Colors::ConflictingTurn))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
