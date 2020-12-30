use geom::{Angle, ArrowCap, Distance, PolyLine, Polygon, Pt2D, Ring};
use map_model::{Map, TurnType};
use sim::{CarID, CarStatus, DrawCarInput, VehicleType};
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};

use crate::colors::ColorScheme;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use crate::{AppLike, ID};

const CAR_WIDTH: Distance = Distance::const_meters(1.75);

pub struct DrawCar {
    pub id: CarID,
    body: PolyLine,
    body_polygon: Polygon,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawCar {
    pub fn new(input: DrawCarInput, map: &Map, prerender: &Prerender, cs: &ColorScheme) -> DrawCar {
        let mut draw_default = GeomBatch::new();

        // Wheels
        for side in vec![
            input.body.shift_right(CAR_WIDTH / 2.0),
            input.body.shift_left(CAR_WIDTH / 2.0),
        ]
        .into_iter()
        .flatten()
        {
            let len = side.length();
            if len <= Distance::meters(2.0) {
                // The original body may be fine, but sometimes shifting drastically shortens the
                // length due to miter threshold chopping. Just give up on wheels in that case
                // instead of crashing.
                continue;
            }
            draw_default.push(
                cs.bike_frame,
                side.exact_slice(Distance::meters(0.5), Distance::meters(1.0))
                    .make_polygons(OUTLINE_THICKNESS / 2.0),
            );
            draw_default.push(
                cs.bike_frame,
                side.exact_slice(len - Distance::meters(2.0), len - Distance::meters(1.5))
                    .make_polygons(OUTLINE_THICKNESS / 2.0),
            );
        }

        let body_polygon = if input.body.length() < Distance::meters(1.1) {
            // Simpler shape while appearing from a border
            input.body.make_polygons(CAR_WIDTH)
        } else {
            let front_corner = input.body.length() - Distance::meters(1.0);
            let thick_line = input
                .body
                .exact_slice(Distance::ZERO, front_corner)
                .make_polygons(CAR_WIDTH);

            let (corner_pt, corner_angle) = input.body.must_dist_along(front_corner);
            let tip_pt = input.body.last_pt();
            let tip_angle = input.body.last_line().angle();
            let front = Ring::must_new(vec![
                corner_pt.project_away(CAR_WIDTH / 2.0, corner_angle.rotate_degs(90.0)),
                corner_pt.project_away(CAR_WIDTH / 2.0, corner_angle.rotate_degs(-90.0)),
                tip_pt.project_away(CAR_WIDTH / 4.0, tip_angle.rotate_degs(-90.0)),
                tip_pt.project_away(CAR_WIDTH / 4.0, tip_angle.rotate_degs(90.0)),
                corner_pt.project_away(CAR_WIDTH / 2.0, corner_angle.rotate_degs(90.0)),
            ])
            .to_polygon();
            front.union(thick_line)
        };

        draw_default.push(zoomed_color_car(&input, cs), body_polygon.clone());
        if input.status == CarStatus::Parked {
            draw_default.append(
                GeomBatch::load_svg(prerender, "system/assets/map/parked_car.svg")
                    .scale(0.01)
                    .centered_on(input.body.middle()),
            );
        }

        if input.show_parking_intent {
            // draw intent bubble
            let bubble_z = -0.0001;
            let mut bubble_batch =
                GeomBatch::load_svg(prerender, "system/assets/map/thought_bubble.svg")
                    .scale(0.05)
                    .centered_on(input.body.middle())
                    .translate(4.0, -4.0)
                    .set_z_offset(bubble_z);

            let intent_batch = GeomBatch::load_svg(prerender, "system/assets/map/parking.svg")
                .scale(0.015)
                .centered_on(input.body.middle())
                .translate(4.5, -4.5)
                .set_z_offset(bubble_z);

            bubble_batch.append(intent_batch);

            draw_default.append(bubble_batch);
        }

        // If the vehicle is temporarily too short for anything, just omit.
        if input.body.length() >= Distance::meters(2.5) {
            let arrow_len = 0.8 * CAR_WIDTH;
            let arrow_thickness = Distance::meters(0.5);

            if let Some(t) = input.waiting_for_turn {
                match map.get_t(t).turn_type {
                    TurnType::Left | TurnType::UTurn => {
                        let (pos, angle) = input
                            .body
                            .must_dist_along(input.body.length() - Distance::meters(2.5));

                        draw_default.push(
                            cs.turn_arrow,
                            PolyLine::must_new(vec![
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(90.0)),
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(-90.0)),
                            ])
                            .make_arrow(arrow_thickness, ArrowCap::Triangle),
                        );
                    }
                    TurnType::Right => {
                        let (pos, angle) = input
                            .body
                            .must_dist_along(input.body.length() - Distance::meters(2.5));

                        draw_default.push(
                            cs.turn_arrow,
                            PolyLine::must_new(vec![
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(-90.0)),
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(90.0)),
                            ])
                            .make_arrow(arrow_thickness, ArrowCap::Triangle),
                        );
                    }
                    TurnType::Straight => {}
                    TurnType::Crosswalk | TurnType::SharedSidewalkCorner => unreachable!(),
                }

                // Always draw the brake light
                let (pos, angle) = input.body.must_dist_along(Distance::meters(0.5));
                // TODO rounded
                let window_length_gap = Distance::meters(0.2);
                let window_thickness = Distance::meters(0.3);
                draw_default.push(
                    cs.brake_light,
                    thick_line_from_angle(
                        window_thickness,
                        CAR_WIDTH - window_length_gap * 2.0,
                        pos.project_away(
                            CAR_WIDTH / 2.0 - window_length_gap,
                            angle.rotate_degs(-90.0),
                        ),
                        angle.rotate_degs(90.0),
                    ),
                );
            }
        }

        if let Some(line) = input.label {
            // If the vehicle is temporarily too short, just skip the label.
            if let Ok((pt, angle)) = input
                .body
                .dist_along(input.body.length() - Distance::meters(3.5))
            {
                draw_default.append(
                    Text::from(Line(line).fg(cs.bus_label))
                        .render_autocropped(prerender)
                        .scale(0.07)
                        .centered_on(pt)
                        .rotate(angle.reorient()),
                );
            }
        }

        // TODO Technically some of the body may need to be at different zorders during
        // transitions, but that's way too much effort
        let zorder = input
            .partly_on
            .into_iter()
            .chain(vec![input.on])
            .map(|on| on.get_zorder(map))
            .max()
            .unwrap();
        DrawCar {
            id: input.id,
            body: input.body,
            body_polygon,
            zorder,
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawCar {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &dyn AppLike, _: &DrawOptions) {
        g.redraw(&self.draw_default);
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.body
            .to_thick_boundary(CAR_WIDTH, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.body_polygon.clone())
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
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
    PolyLine::must_new(vec![pt, pt2]).make_polygons(thickness)
}

fn zoomed_color_car(input: &DrawCarInput, cs: &ColorScheme) -> Color {
    if input.id.1 == VehicleType::Bus {
        cs.bus_body
    } else if input.id.1 == VehicleType::Train {
        cs.train_body
    } else {
        match input.status {
            CarStatus::Moving => cs.rotating_color_agents(input.id.0),
            CarStatus::Parked => cs.parked_car,
        }
    }
}
