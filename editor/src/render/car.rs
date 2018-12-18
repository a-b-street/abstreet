use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, Circle, PolyLine, Polygon, Pt2D};
use map_model::{Map, TurnType};
use sim::{CarID, CarState, DrawCarInput};
use std;

const CAR_WIDTH: f64 = 2.0;

pub struct DrawCar {
    pub id: CarID,
    body_polygon: Polygon,
    window_polygons: Vec<Polygon>,
    left_blinker: Circle,
    right_blinker: Circle,
    left_blinker_on: bool,
    right_blinker_on: bool,
    // TODO maybe also draw lookahead buffer to know what the car is considering
    stopping_buffer: Option<Polygon>,
    state: CarState,
}

impl DrawCar {
    pub fn new(input: DrawCarInput, map: &Map) -> DrawCar {
        let (left_blinker_on, right_blinker_on) = if let Some(t) = input.waiting_for_turn {
            match map.get_t(t).turn_type {
                TurnType::Left => (true, false),
                TurnType::Right => (false, true),
                TurnType::Straight => (true, true),
                _ => unreachable!(),
            }
        } else {
            (false, false)
        };

        let stopping_buffer = input
            .stopping_trace
            .map(|t| t.make_polygons_blindly(CAR_WIDTH));

        let front_window_length_gap = 0.2;
        let front_window_thickness = 0.3;

        DrawCar {
            id: input.id,
            body_polygon: thick_line_from_angle(
                CAR_WIDTH,
                input.vehicle_length.value_unsafe,
                input.front,
                // find the back of the car relative to the front
                input.angle.opposite(),
            ),
            // TODO it's way too hard to understand and tune this. just wait and stick in sprites
            // or something.
            window_polygons: vec![
                thick_line_from_angle(
                    front_window_thickness,
                    CAR_WIDTH - 2.0 * front_window_length_gap,
                    input
                        .front
                        .project_away(1.0, input.angle.opposite())
                        .project_away(
                            CAR_WIDTH / 2.0 - front_window_length_gap,
                            input.angle.rotate_degs(-90.0),
                        ),
                    input.angle.rotate_degs(90.0),
                ),
                thick_line_from_angle(
                    front_window_thickness * 0.8,
                    CAR_WIDTH - 2.0 * front_window_length_gap,
                    input
                        .front
                        .project_away(
                            input.vehicle_length.value_unsafe - 1.0,
                            input.angle.opposite(),
                        )
                        .project_away(
                            CAR_WIDTH / 2.0 - front_window_length_gap,
                            input.angle.rotate_degs(-90.0),
                        ),
                    input.angle.rotate_degs(90.0),
                ),
            ],
            left_blinker: Circle::new(
                input
                    .front
                    .project_away(
                        input.vehicle_length.value_unsafe - 0.5,
                        input.angle.opposite(),
                    )
                    .project_away(CAR_WIDTH / 2.0 - 0.5, input.angle.rotate_degs(-90.0)),
                0.2,
            ),
            right_blinker: Circle::new(
                input
                    .front
                    .project_away(
                        input.vehicle_length.value_unsafe - 0.5,
                        input.angle.opposite(),
                    )
                    .project_away(CAR_WIDTH / 2.0 - 0.5, input.angle.rotate_degs(90.0)),
                0.2,
            ),
            left_blinker_on,
            right_blinker_on,
            stopping_buffer,
            state: input.state,
        }
    }
}

impl Renderable for DrawCar {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            // TODO if it's a bus, color it differently -- but how? :\
            match self.state {
                CarState::Debug => ctx
                    .cs
                    .get_def("debug car", Color::rgba(0, 0, 255, 0.8))
                    .shift(self.id.0),
                CarState::Moving => ctx.cs.get_def("moving car", Color::CYAN).shift(self.id.0),
                CarState::Stuck => ctx.cs.get_def("stuck car", Color::RED).shift(self.id.0),
                CarState::Parked => ctx
                    .cs
                    .get_def("parked car", Color::rgb(180, 233, 76))
                    .shift(self.id.0),
            }
        });
        g.draw_polygon(color, &self.body_polygon);
        for p in &self.window_polygons {
            g.draw_polygon(ctx.cs.get_def("car window", Color::BLACK), p);
        }

        let blinker_on = ctx.cs.get_def("blinker on", Color::BLACK);
        // Don't use the simulation time, because then fast simulations would have cars blinking
        // _very_ fast or slow. Unless that's what people expect?
        let mut any_blinkers_on = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_millis()
            % 300
            < 150;
        // But if a car is trying to go straight, don't blink at all.
        if self.left_blinker_on && self.right_blinker_on {
            any_blinkers_on = true;
        }
        if any_blinkers_on && self.left_blinker_on {
            g.draw_circle(blinker_on, &self.left_blinker);
        }
        if any_blinkers_on && self.right_blinker_on {
            g.draw_circle(blinker_on, &self.right_blinker);
        }

        if opts.debug_mode {
            if let Some(ref t) = self.stopping_buffer {
                g.draw_polygon(
                    ctx.cs
                        .get_def("car stopping buffer", Color::rgba(255, 0, 0, 0.7)),
                    t,
                );
            }
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.body_polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.body_polygon.contains_pt(pt)
    }
}

fn thick_line_from_angle(thickness: f64, line_length: f64, pt: Pt2D, angle: Angle) -> Polygon {
    let pt2 = pt.project_away(line_length, angle);
    // Shouldn't ever fail for a single line
    PolyLine::new(vec![pt, pt2]).make_polygons_blindly(thickness)
}
