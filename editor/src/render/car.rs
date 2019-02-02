use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Angle, Bounds, Circle, Distance, PolyLine, Polygon, Pt2D};
use map_model::{Map, TurnType};
use sim::{CarID, CarState, DrawCarInput};
use std;

const CAR_WIDTH: Distance = Distance::const_meters(2.0);

pub struct DrawCar {
    pub id: CarID,
    body_polygon: Polygon,
    // Optional and could be empty for super short cars near borders.
    window_polygons: Vec<Polygon>,
    left_blinkers: Option<(Circle, Circle)>,
    right_blinkers: Option<(Circle, Circle)>,
    left_blinker_on: bool,
    right_blinker_on: bool,
    // TODO maybe also draw lookahead buffer to know what the car is considering
    stopping_buffer: Option<Polygon>,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawCar {
    pub fn new(input: DrawCarInput, map: &Map, prerender: &Prerender, cs: &ColorScheme) -> DrawCar {
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

        let stopping_buffer = input.stopping_trace.map(|t| t.make_polygons(CAR_WIDTH));

        let (front_blinker_pos, front_blinker_angle) = input
            .body
            .dist_along(input.body.length() - Distance::meters(0.5));
        let (back_blinker_pos, back_blinker_angle) = input.body.dist_along(Distance::meters(0.5));
        let blinker_radius = Distance::meters(0.3);

        let window_length_gap = Distance::meters(0.2);
        let window_thickness = Distance::meters(0.3);
        let front_window = {
            let (pos, angle) = input
                .body
                .dist_along(input.body.length() - Distance::meters(1.0));
            thick_line_from_angle(
                window_thickness,
                CAR_WIDTH - window_length_gap * 2.0,
                pos.project_away(
                    CAR_WIDTH / 2.0 - window_length_gap,
                    angle.rotate_degs(-90.0),
                ),
                angle.rotate_degs(90.0),
            )
        };
        let back_window = {
            let (pos, angle) = input.body.dist_along(Distance::meters(1.0));
            thick_line_from_angle(
                window_thickness * 0.8,
                CAR_WIDTH - window_length_gap * 2.0,
                pos.project_away(
                    CAR_WIDTH / 2.0 - window_length_gap,
                    angle.rotate_degs(-90.0),
                ),
                angle.rotate_degs(90.0),
            )
        };

        let body_polygon = input.body.make_polygons(CAR_WIDTH);

        let draw_default = prerender.upload_borrowed(vec![
            (
                // TODO if it's a bus, color it differently -- but how? :\
                match input.state {
                    CarState::Debug => cs
                        .get_def("debug car", Color::BLUE.alpha(0.8))
                        .shift(input.id.0),
                    CarState::Moving => cs.get_def("moving car", Color::CYAN).shift(input.id.0),
                    CarState::Stuck => cs
                        .get_def("stuck car", Color::rgb_f(0.9, 0.0, 0.0))
                        .shift(input.id.0),
                    CarState::Parked => cs
                        .get_def("parked car", Color::rgb(180, 233, 76))
                        .shift(input.id.0),
                },
                &body_polygon,
            ),
            (cs.get_def("car window", Color::BLACK), &front_window),
            (cs.get("car window"), &back_window),
        ]);

        DrawCar {
            id: input.id,
            body_polygon,
            window_polygons: vec![front_window, back_window],
            left_blinkers: Some((
                Circle::new(
                    front_blinker_pos.project_away(
                        CAR_WIDTH / 2.0 - Distance::meters(0.5),
                        front_blinker_angle.rotate_degs(-90.0),
                    ),
                    blinker_radius,
                ),
                Circle::new(
                    back_blinker_pos.project_away(
                        CAR_WIDTH / 2.0 - Distance::meters(0.5),
                        back_blinker_angle.rotate_degs(-90.0),
                    ),
                    blinker_radius,
                ),
            )),
            right_blinkers: Some((
                Circle::new(
                    front_blinker_pos.project_away(
                        CAR_WIDTH / 2.0 - Distance::meters(0.5),
                        front_blinker_angle.rotate_degs(90.0),
                    ),
                    blinker_radius,
                ),
                Circle::new(
                    back_blinker_pos.project_away(
                        CAR_WIDTH / 2.0 - Distance::meters(0.5),
                        back_blinker_angle.rotate_degs(90.0),
                    ),
                    blinker_radius,
                ),
            )),
            left_blinker_on,
            right_blinker_on,
            stopping_buffer,
            zorder: input.on.get_zorder(map),
            draw_default,
        }
    }
}

impl Renderable for DrawCar {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        if let Some(color) = opts.color {
            let mut draw = vec![(color, &self.body_polygon)];
            for p in &self.window_polygons {
                draw.push((ctx.cs.get("car window"), p));
            }
            g.draw_polygon_batch(draw);
        } else {
            g.redraw(&self.draw_default);
        }

        let blinker_on = ctx.cs.get_def("blinker on", Color::RED);
        // Don't use the simulation time, because then fast simulations would have cars blinking
        // _very_ fast or slow. Unless that's what people expect?
        // But if a car is trying to go straight, don't blink at all.
        let any_blinkers_on = if self.left_blinker_on && self.right_blinker_on {
            true
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .subsec_millis()
                % 300
                < 150
        };
        if any_blinkers_on {
            // If both are on, don't show the front ones -- just the back brake lights
            if let (Some(left_blinkers), Some(right_blinkers)) =
                (&self.left_blinkers, &self.right_blinkers)
            {
                if self.left_blinker_on {
                    if !self.right_blinker_on {
                        g.draw_circle(blinker_on, &left_blinkers.0);
                    }
                    g.draw_circle(blinker_on, &left_blinkers.1);
                }
                if self.right_blinker_on {
                    if !self.left_blinker_on {
                        g.draw_circle(blinker_on, &right_blinkers.0);
                    }
                    g.draw_circle(blinker_on, &right_blinkers.1);
                }
            }
        }

        if opts.debug_mode {
            if let Some(ref t) = self.stopping_buffer {
                g.draw_polygon(
                    ctx.cs.get_def("car stopping buffer", Color::RED.alpha(0.7)),
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

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

fn thick_line_from_angle(
    thickness: Distance,
    line_length: Distance,
    pt: Pt2D,
    angle: Angle,
) -> Polygon {
    let pt2 = pt.project_away(line_length, angle);
    // Shouldn't ever fail for a single line
    PolyLine::new(vec![pt, pt2]).make_polygons(thickness)
}
