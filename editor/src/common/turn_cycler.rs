use crate::objects::{DrawCtx, ID};
use crate::render::{draw_signal_diagram, DrawTurn};
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, Key};
use geom::Duration;
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

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) {
        match ui.state.primary.current_selection {
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
                    if !ui.state.primary.map.get_turns_from_lane(id).is_empty()
                        && ctx
                            .input
                            .key_pressed(Key::Tab, "cycle through this lane's turns")
                    {
                        self.state = State::CycleTurns(id, 0);
                    }
                }

                // TODO...
                //ctx.hints.suppress_traffic_signal_details = Some(ctx.primary.map.get_l(id).dst_i);
            }
            Some(ID::Intersection(id)) => {
                self.state = State::ShowIntersection(id);
            }
            _ => {
                self.state = State::Inactive;
            }
        }

        // TODO I think it's possible for this state to get out of sync with reality, by holding
        // the key while changing to a mode that doesn't invoke CommonState.
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

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        match self.state {
            State::Inactive => {}
            State::ShowLane(l) => {
                for turn in &ui.state.primary.map.get_turns_from_lane(l) {
                    DrawTurn::draw_full(turn, g, color_turn_type(turn.turn_type, ui).alpha(0.5));
                }
            }
            State::CycleTurns(l, idx) => {
                let turns = ui.state.primary.map.get_turns_from_lane(l);
                let t = turns[idx % turns.len()];
                DrawTurn::draw_full(t, g, color_turn_type(t.turn_type, ui));
            }
            State::ShowIntersection(i) => {
                if self.shift_key_held {
                    if let Some(signal) = ui.state.primary.map.maybe_get_traffic_signal(i) {
                        let (cycle, mut time_left) =
                            signal.current_cycle_and_remaining_time(ui.state.primary.sim.time());
                        if ui
                            .state
                            .primary
                            .sim
                            .is_in_overtime(i, &ui.state.primary.map)
                        {
                            // TODO Hacky way of indicating overtime. Should make a 3-case enum.
                            time_left = Duration::seconds(-1.0);
                        }
                        let ctx = DrawCtx {
                            cs: &ui.state.cs,
                            map: &ui.state.primary.map,
                            draw_map: &ui.state.primary.draw_map,
                            sim: &ui.state.primary.sim,
                            hints: &ui.hints,
                        };
                        draw_signal_diagram(
                            i,
                            cycle.idx,
                            Some(time_left),
                            g.canvas.top_menu_height() + 10.0,
                            g,
                            &ctx,
                        );
                    }
                }
            }
        }
    }
}

fn color_turn_type(t: TurnType, ui: &UI) -> Color {
    match t {
        TurnType::SharedSidewalkCorner => ui
            .state
            .cs
            .get_def("shared sidewalk corner turn", Color::BLACK),
        TurnType::Crosswalk => ui.state.cs.get_def("crosswalk turn", Color::WHITE),
        TurnType::Straight => ui.state.cs.get_def("straight turn", Color::BLUE),
        TurnType::Right => ui.state.cs.get_def("right turn", Color::GREEN),
        TurnType::Left => ui.state.cs.get_def("left turn", Color::RED),
    }
}
