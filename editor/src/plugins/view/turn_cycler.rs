use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{draw_signal_diagram, DrawTurn};
use dimensioned::si;
use ezgui::{Color, GfxCtx, Key, TOP_MENU_HEIGHT};
use map_model::{IntersectionID, LaneID, TurnType};

pub struct TurnCyclerState {
    state: State,
    shift_key_held: bool,
}

enum State {
    Inactive,
    ShowLane(LaneID),
    CycleTurns(LaneID, usize),
    ShowIntersection(IntersectionID),
}

impl TurnCyclerState {
    pub fn new() -> TurnCyclerState {
        TurnCyclerState {
            state: State::Inactive,
            shift_key_held: false,
        }
    }
}

impl Plugin for TurnCyclerState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match ctx.primary.current_selection {
            Some(ID::Lane(id)) => {
                if let State::CycleTurns(current, idx) = self.state {
                    if current != id {
                        self.state = State::ShowLane(id);
                    } else if ctx
                        .input
                        .key_pressed(Key::Tab, "cycle through this lane's turns")
                    {
                        self.state = State::CycleTurns(id, idx + 1);
                    }
                } else {
                    self.state = State::ShowLane(id);
                    if !ctx.primary.map.get_turns_from_lane(id).is_empty()
                        && ctx
                            .input
                            .key_pressed(Key::Tab, "cycle through this lane's turns")
                    {
                        self.state = State::CycleTurns(id, 0);
                    }
                }

                ctx.hints.suppress_traffic_signal_details = Some(ctx.primary.map.get_l(id).dst_i);
            }
            Some(ID::Intersection(id)) => {
                self.state = State::ShowIntersection(id);
            }
            _ => {
                self.state = State::Inactive;
            }
        }

        if self.shift_key_held {
            if ctx.input.key_released(Key::LeftShift) {
                self.shift_key_held = false;
            }
        } else {
            // TODO How to tell the user that holding control and shift is sometimes useful?
            if ctx
                .input
                .unimportant_key_pressed(Key::LeftShift, "show full traffic signal diagram")
            {
                self.shift_key_held = true;
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self.state {
            State::Inactive => {}
            State::ShowLane(l) => {
                for turn in &ctx.map.get_turns_from_lane(l) {
                    DrawTurn::draw_full(turn, g, color_turn_type(turn.turn_type, ctx).alpha(0.5));
                }
            }
            State::CycleTurns(l, idx) => {
                let turns = ctx.map.get_turns_from_lane(l);
                let t = turns[idx % turns.len()];
                DrawTurn::draw_full(t, g, color_turn_type(t.turn_type, ctx));
            }
            State::ShowIntersection(i) => {
                if self.shift_key_held {
                    if let Some(signal) = ctx.map.maybe_get_traffic_signal(i) {
                        let (cycle, mut time_left) =
                            signal.current_cycle_and_remaining_time(ctx.sim.time.as_time());
                        if ctx.sim.is_in_overtime(i) {
                            // TODO Hacky way of indicating overtime. Should make a 3-case enum.
                            time_left = -1.0 * si::S;
                        }
                        draw_signal_diagram(
                            i,
                            cycle.idx,
                            Some(time_left),
                            TOP_MENU_HEIGHT + 10.0,
                            g,
                            ctx,
                        );
                    }
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
