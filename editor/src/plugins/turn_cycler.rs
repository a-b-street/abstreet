// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::{GfxCtx, UserInput};
use map_model::{IntersectionID, LaneID, Map};
use piston::input::Key;
use plugins::selection::SelectionState;
use render::{DrawMap, Renderable};
use sim::Sim;

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

    pub fn event(&mut self, input: &mut UserInput, current_selection: &SelectionState) -> bool {
        let current_id = match current_selection {
            SelectionState::SelectedLane(id) => *id,
            SelectionState::SelectedIntersection(id) => {
                *self = TurnCyclerState::Intersection(*id);
                return false;
            }
            _ => {
                *self = TurnCyclerState::Inactive;
                return false;
            }
        };

        let mut new_state: Option<TurnCyclerState> = None;
        let active = match self {
            TurnCyclerState::Inactive | TurnCyclerState::Intersection(_) => {
                new_state = Some(TurnCyclerState::Active(current_id, None));
                false
            }
            TurnCyclerState::Active(old_id, current_turn_index) => {
                if current_id != *old_id {
                    new_state = Some(TurnCyclerState::Inactive);
                    false
                } else if input.key_pressed(Key::Tab, "cycle through this lane's turns") {
                    let idx = match *current_turn_index {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    new_state = Some(TurnCyclerState::Active(current_id, Some(idx)));
                    true
                } else {
                    false
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }

    pub fn draw(
        &self,
        map: &Map,
        draw_map: &DrawMap,
        control_map: &ControlMap,
        sim: &Sim,
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

                            for t in map.get_turns_in_intersection(turn.parent) {
                                if t.conflicts_with(turn) {
                                    let draw_t = draw_map.get_t(t.id);
                                    // TODO should we instead change color_t?
                                    draw_t.draw(g, cs.get(Colors::ConflictingTurn), cs);
                                }
                            }
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
                    let (cycle, _) = signal.current_cycle_and_remaining_time(sim.time.as_time());
                    for t in &cycle.turns {
                        draw_map.get_t(*t).draw_full(g, cs.get(Colors::Turn));
                    }
                }
            }
        }
    }
}
