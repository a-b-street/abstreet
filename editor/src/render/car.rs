use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender};
use geom::{Angle, Circle, Distance, PolyLine, Polygon, Pt2D};
use map_model::{Map, TurnType};
use sim::{CarID, CarStatus, DrawCarInput};

const CAR_WIDTH: Distance = Distance::const_meters(2.0);

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

        let body_polygon = input.body.make_polygons(CAR_WIDTH);
        draw_default.push(
            // TODO if it's a bus, color it differently -- but how? :\
            match input.status {
                CarStatus::Debug => cs.get_def("debug car", Color::BLUE.alpha(0.8)),
                CarStatus::Moving => cs.get_def("moving car", Color::CYAN),
                CarStatus::Stuck => cs.get_def("stuck car", Color::rgb(222, 184, 135)),
                CarStatus::Parked => cs.get_def("parked car", Color::rgb(180, 233, 76)),
            },
            body_polygon.clone(),
        );

        {
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
            draw_default.push(cs.get_def("car window", Color::BLACK), front_window);
            draw_default.push(cs.get("car window"), back_window);
        }

        {
            let radius = Distance::meters(0.3);
            let edge_offset = Distance::meters(0.5);
            let (front_pos, front_angle) = input.body.dist_along(input.body.length() - edge_offset);
            let (back_pos, back_angle) = input.body.dist_along(edge_offset);

            let front_left = Circle::new(
                front_pos.project_away(
                    CAR_WIDTH / 2.0 - edge_offset,
                    front_angle.rotate_degs(-90.0),
                ),
                radius,
            );
            let front_right = Circle::new(
                front_pos
                    .project_away(CAR_WIDTH / 2.0 - edge_offset, front_angle.rotate_degs(90.0)),
                radius,
            );
            let back_left = Circle::new(
                back_pos.project_away(CAR_WIDTH / 2.0 - edge_offset, back_angle.rotate_degs(-90.0)),
                radius,
            );
            let back_right = Circle::new(
                back_pos.project_away(CAR_WIDTH / 2.0 - edge_offset, back_angle.rotate_degs(90.0)),
                radius,
            );

            let bg_color = cs.get_def("blinker background", Color::grey(0.2));
            for c in vec![&front_left, &front_right, &back_left, &back_right] {
                draw_default.push(bg_color, c.to_polygon());
            }

            let arrow_color = cs.get_def("blinker on", Color::RED);
            if let Some(t) = input.waiting_for_turn {
                let turn = map.get_t(t);
                let angle = turn.angle();
                match turn.turn_type {
                    TurnType::Left | TurnType::LaneChangeLeft => {
                        for circle in vec![front_left, back_left] {
                            draw_default.push(
                                arrow_color,
                                PolyLine::new(vec![
                                    circle.center.project_away(radius / 2.0, angle.opposite()),
                                    circle.center.project_away(radius / 2.0, angle),
                                ])
                                .make_arrow(Distance::meters(0.15))
                                .unwrap(),
                            );
                        }
                    }
                    TurnType::Right | TurnType::LaneChangeRight => {
                        for circle in vec![front_right, back_right] {
                            draw_default.push(
                                arrow_color,
                                PolyLine::new(vec![
                                    circle.center.project_away(radius / 2.0, angle.opposite()),
                                    circle.center.project_away(radius / 2.0, angle),
                                ])
                                .make_arrow(Distance::meters(0.15))
                                .unwrap(),
                            );
                        }
                    }
                    TurnType::Straight => {
                        draw_default.push(arrow_color, back_left.to_polygon());
                        draw_default.push(arrow_color, back_right.to_polygon());
                    }
                    TurnType::Crosswalk | TurnType::SharedSidewalkCorner => unreachable!(),
                }
            }
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

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &self.body_polygon);
        } else {
            g.redraw(&self.draw_default);
        }
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
