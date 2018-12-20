use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::DrawTurn;
use ezgui::{Color, GfxCtx, Key};
use map_model::{LaneID, TurnType};

pub enum TurnCyclerState {
    Inactive,
    ShowLane(LaneID),
    CycleTurns(LaneID, usize),
}

impl TurnCyclerState {
    pub fn new() -> TurnCyclerState {
        TurnCyclerState::Inactive
    }
}

impl Plugin for TurnCyclerState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match ctx.primary.current_selection {
            Some(ID::Lane(id)) => {
                if let TurnCyclerState::CycleTurns(current, idx) = self {
                    if *current != id {
                        *self = TurnCyclerState::ShowLane(id);
                    } else if ctx
                        .input
                        .key_pressed(Key::Tab, "cycle through this lane's turns")
                    {
                        *self = TurnCyclerState::CycleTurns(id, *idx + 1);
                    }
                } else {
                    *self = TurnCyclerState::ShowLane(id);
                    if ctx
                        .input
                        .key_pressed(Key::Tab, "cycle through this lane's turns")
                    {
                        *self = TurnCyclerState::CycleTurns(id, 0);
                    }
                }
            }
            _ => {
                *self = TurnCyclerState::Inactive;
            }
        };
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::ShowLane(l) => {
                for turn in &ctx.map.get_turns_from_lane(*l) {
                    let color = match turn.turn_type {
                        TurnType::SharedSidewalkCorner => {
                            ctx.cs.get_def("shared sidewalk corner turn", Color::BLACK)
                        }
                        TurnType::Crosswalk => ctx.cs.get_def("crosswalk turn", Color::WHITE),
                        TurnType::Straight => ctx.cs.get_def("straight turn", Color::BLUE),
                        TurnType::Right => ctx.cs.get_def("right turn", Color::GREEN),
                        TurnType::Left => ctx.cs.get_def("left turn", Color::RED),
                    }
                    .alpha(0.5);
                    DrawTurn::draw_full(turn, g, color);
                }
            }
            TurnCyclerState::CycleTurns(l, idx) => {
                let turns = ctx.map.get_turns_from_lane(*l);
                if !turns.is_empty() {
                    DrawTurn::draw_full(
                        turns[*idx % turns.len()],
                        g,
                        ctx.cs.get_def("current selected turn", Color::RED),
                    );
                }
            }
        }
    }
}
