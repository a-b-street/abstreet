use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, RewriteColor, Text};
use geom::{Angle, ArrowCap, Distance, PolyLine, Polygon, Pt2D};
use map_model::{Map, TurnType};
use sim::{CarID, CarStatus, DrawCarInput, VehicleType};

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
            input.body.shift_right(CAR_WIDTH / 2.0).unwrap(),
            input.body.shift_left(CAR_WIDTH / 2.0).unwrap(),
        ] {
            let len = side.length();
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

        let body_polygon = {
            let len = input.body.length();
            let front_corner = len - Distance::meters(1.0);
            let thick_line = input
                .body
                .exact_slice(Distance::ZERO, front_corner)
                .make_polygons(CAR_WIDTH);

            let (corner_pt, corner_angle) = input.body.dist_along(front_corner);
            let (tip_pt, tip_angle) = input.body.dist_along(len);
            let front = Polygon::new(&vec![
                corner_pt.project_away(CAR_WIDTH / 2.0, corner_angle.rotate_degs(90.0)),
                corner_pt.project_away(CAR_WIDTH / 2.0, corner_angle.rotate_degs(-90.0)),
                tip_pt.project_away(CAR_WIDTH / 4.0, tip_angle.rotate_degs(-90.0)),
                tip_pt.project_away(CAR_WIDTH / 4.0, tip_angle.rotate_degs(90.0)),
            ]);
            front.union(thick_line)
        };

        draw_default.push(zoomed_color_car(&input, cs), body_polygon.clone());
        if input.status == CarStatus::Parked {
            draw_default.add_svg(
                prerender,
                "../data/system/assets/map/parked_car.svg",
                input.body.middle(),
                0.01,
                Angle::ZERO,
                RewriteColor::NoOp,
                true,
            );
        }

        {
            let arrow_len = 0.8 * CAR_WIDTH;
            let arrow_thickness = Distance::meters(0.5);

            if let Some(t) = input.waiting_for_turn {
                match map.get_t(t).turn_type {
                    TurnType::Left => {
                        let (pos, angle) = input
                            .body
                            .dist_along(input.body.length() - Distance::meters(2.5));

                        draw_default.push(
                            cs.turn_arrow,
                            PolyLine::new(vec![
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(90.0)),
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(-90.0)),
                            ])
                            .make_arrow(arrow_thickness, ArrowCap::Triangle)
                            .unwrap(),
                        );
                    }
                    TurnType::Right => {
                        let (pos, angle) = input
                            .body
                            .dist_along(input.body.length() - Distance::meters(2.5));

                        draw_default.push(
                            cs.turn_arrow,
                            PolyLine::new(vec![
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(-90.0)),
                                pos.project_away(arrow_len / 2.0, angle.rotate_degs(90.0)),
                            ])
                            .make_arrow(arrow_thickness, ArrowCap::Triangle)
                            .unwrap(),
                        );
                    }
                    TurnType::Straight | TurnType::LaneChangeLeft | TurnType::LaneChangeRight => {}
                    TurnType::Crosswalk | TurnType::SharedSidewalkCorner => unreachable!(),
                }

                // Always draw the brake light
                let (pos, angle) = input.body.dist_along(Distance::meters(0.5));
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
            // TODO Would rotation make any sense? Or at least adjust position/size while turning.
            // Buses are a constant length, so hardcoding this is fine.
            draw_default.add_transformed(
                Text::from(Line(line).fg(cs.bus_label)).render_to_batch(prerender),
                input.body.dist_along(Distance::meters(9.0)).0,
                0.07,
                Angle::ZERO,
                RewriteColor::NoOp,
            );
        }

        DrawCar {
            id: input.id,
            body: input.body,
            body_polygon,
            zorder: input.on.get_zorder(map),
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawCar {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, _: &DrawOptions) {
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
    PolyLine::new(vec![pt, pt2]).make_polygons(thickness)
}

fn zoomed_color_car(input: &DrawCarInput, cs: &ColorScheme) -> Color {
    if input.id.1 == VehicleType::Bus {
        cs.bus_body
    } else {
        match input.status {
            CarStatus::Moving => cs.rotating_color_agents(input.id.0),
            CarStatus::Parked => cs.rotating_color_agents(input.id.0).fade(1.5),
        }
    }
}
