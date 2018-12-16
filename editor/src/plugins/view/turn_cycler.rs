use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{draw_signal_cycle, draw_stop_sign, stop_sign_rendering_hints, DrawTurn};
use ezgui::{Color, GfxCtx, Key, Text};
use geom::{Polygon, Pt2D};
use map_model::{IntersectionID, LaneID, TurnType};

pub struct TurnCyclerState {
    state: State,
    key: Key,
}

enum State {
    Inactive,
    ShowLane(LaneID),
    CycleTurns(LaneID, usize),
    ShowIntersection(IntersectionID),
}

impl TurnCyclerState {
    pub fn new(key: Key) -> TurnCyclerState {
        TurnCyclerState {
            key,
            state: State::Inactive,
        }
    }
}

impl Plugin for TurnCyclerState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match ctx.primary.current_selection {
            Some(ID::Intersection(id)) => {
                self.state = State::ShowIntersection(id);

                if let Some(signal) = ctx.primary.map.maybe_get_traffic_signal(id) {
                    ctx.hints.suppress_intersection_icon = Some(id);
                    if !ctx.primary.sim.is_in_overtime(id) {
                        let (cycle, _) =
                            signal.current_cycle_and_remaining_time(ctx.primary.sim.time.as_time());
                        ctx.hints
                            .hide_crosswalks
                            .extend(cycle.get_absent_crosswalks(&ctx.primary.map));
                    }
                } else if let Some(sign) = ctx.primary.map.maybe_get_stop_sign(id) {
                    stop_sign_rendering_hints(&mut ctx.hints, sign, &ctx.primary.map, ctx.cs);
                }
            }
            Some(ID::Lane(id)) => {
                if let State::CycleTurns(current, idx) = self.state {
                    if current != id {
                        self.state = State::ShowLane(id);
                    } else if ctx
                        .input
                        .key_pressed(self.key, "cycle through this lane's turns")
                    {
                        self.state = State::CycleTurns(id, idx + 1);
                    }
                } else {
                    self.state = State::ShowLane(id);
                    if ctx
                        .input
                        .key_pressed(self.key, "cycle through this lane's turns")
                    {
                        self.state = State::CycleTurns(id, 0);
                    }
                }
            }
            _ => {
                self.state = State::Inactive;
            }
        };
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self.state {
            State::Inactive => {}
            State::ShowLane(l) => {
                for turn in &ctx.map.get_turns_from_lane(l) {
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
            State::CycleTurns(l, idx) => {
                let turns = ctx.map.get_turns_from_lane(l);
                if !turns.is_empty() {
                    DrawTurn::draw_full(
                        turns[idx % turns.len()],
                        g,
                        ctx.cs.get_def("current selected turn", Color::RED),
                    );
                }
            }
            State::ShowIntersection(id) => {
                if let Some(signal) = ctx.map.maybe_get_traffic_signal(id) {
                    if ctx.sim.is_in_overtime(id) {
                        let old_ctx = g.fork_screenspace();
                        let width = 50.0;
                        let height = 100.0;
                        g.draw_polygon(
                            ctx.cs.get_def("signal overtime timer", Color::PINK),
                            &Polygon::rectangle_topleft(Pt2D::new(10.0, 10.0), width, height),
                        );
                        // TODO We can't use draw_text_at, because canvas doesn't know about forked
                        // contexts.
                        ctx.canvas.draw_text_at_screenspace_topleft(
                            g,
                            Text::from_line("Overtime!".to_string()),
                            (10.0 + width / 2.0, 10.0 + height / 2.0),
                        );
                        g.unfork(old_ctx);
                    } else {
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
                                ctx.cs.get_def("timer foreground", Color::RED),
                                &Polygon::rectangle_topleft(Pt2D::new(10.0, 10.0), width, height),
                            );
                            g.draw_polygon(
                                ctx.cs.get_def("timer background", Color::BLACK),
                                &Polygon::rectangle_topleft(
                                    Pt2D::new(10.0, 10.0),
                                    width,
                                    (time_left / cycle.duration).value_unsafe * height,
                                ),
                            );
                            g.unfork(old_ctx);
                        }
                    }
                } else if let Some(sign) = ctx.map.maybe_get_stop_sign(id) {
                    draw_stop_sign(sign, g, ctx.cs, ctx.map);
                }
            }
        }
    }
}
