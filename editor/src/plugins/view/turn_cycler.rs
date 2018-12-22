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

                ctx.hints.suppress_traffic_signal_details = Some(ctx.primary.map.get_l(id).dst_i);
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
                    DrawTurn::draw_full(turn, g, color_turn_type(turn.turn_type, ctx).alpha(0.5));
                }
            }
            TurnCyclerState::CycleTurns(l, idx) => {
                let turns = ctx.map.get_turns_from_lane(*l);
                let t = turns[*idx % turns.len()];
                if !turns.is_empty() {
                    DrawTurn::draw_full(t, g, color_turn_type(t.turn_type, ctx));
                }
            }
        }
    }
}

fn color_turn_type(t: TurnType, ctx: &Ctx) -> Color {
    match t {
        TurnType::SharedSidewalkCorner => {
            ctx.cs.get_def("shared sidewalk corner turn", Color::BLACK)
        }
        TurnType::Crosswalk => ctx.cs.get_def("crosswalk turn", Color::WHITE),
        TurnType::Straight => ctx.cs.get_def("straight turn", Color::BLUE),
        TurnType::Right => ctx.cs.get_def("right turn", Color::GREEN),
        TurnType::Left => ctx.cs.get_def("left turn", Color::RED),
    }
}
