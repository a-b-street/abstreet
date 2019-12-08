use crate::helpers::{ColorScheme, ID};
use crate::render::{AgentColorScheme, DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Angle, Circle, Distance, PolyLine, Polygon, Pt2D};
use map_model::{Map, TurnType};
use sim::{CarID, DrawCarInput};

const CAR_WIDTH: Distance = Distance::const_meters(1.75);

pub struct DrawCar {
    pub id: CarID,
    body: PolyLine,
    body_polygon: Polygon,
    zorder: isize,
    label: Option<Text>,

    draw_default: Drawable,
}

impl DrawCar {
    pub fn new(
        input: DrawCarInput,
        map: &Map,
        prerender: &Prerender,
        cs: &ColorScheme,
        acs: AgentColorScheme,
    ) -> DrawCar {
        let mut draw_default = GeomBatch::new();
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

        draw_default.push(acs.zoomed_color_car(&input, cs), body_polygon.clone());

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
            let arrow_len = 2.0 * radius;
            let edge_offset = Distance::meters(0.5);
            let (back_pos, back_angle) = input.body.dist_along(edge_offset);

            let back_left = Circle::new(
                back_pos.project_away(CAR_WIDTH / 2.0 - edge_offset, back_angle.rotate_degs(-90.0)),
                radius,
            );
            let back_right = Circle::new(
                back_pos.project_away(CAR_WIDTH / 2.0 - edge_offset, back_angle.rotate_degs(90.0)),
                radius,
            );

            let bg_color = cs.get_def("blinker background", Color::grey(0.2));

            let arrow_color = cs.get_def("blinker on", Color::RED);
            if let Some(t) = input.waiting_for_turn {
                match map.get_t(t).turn_type {
                    TurnType::Left | TurnType::LaneChangeLeft => {
                        let angle = back_angle.rotate_degs(-90.0);
                        draw_default.push(
                            arrow_color,
                            PolyLine::new(vec![
                                back_left
                                    .center
                                    .project_away(arrow_len / 2.0, angle.opposite()),
                                back_left.center.project_away(arrow_len / 2.0, angle),
                            ])
                            .make_arrow(Distance::meters(0.15))
                            .unwrap(),
                        );
                    }
                    TurnType::Right | TurnType::LaneChangeRight => {
                        let angle = back_angle.rotate_degs(90.0);
                        draw_default.push(
                            arrow_color,
                            PolyLine::new(vec![
                                back_right
                                    .center
                                    .project_away(arrow_len / 2.0, angle.opposite()),
                                back_right.center.project_away(arrow_len / 2.0, angle),
                            ])
                            .make_arrow(Distance::meters(0.15))
                            .unwrap(),
                        );
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
            label: input
                .label
                .map(|line| Text::from(Line(line).fg(Color::rgb(249, 206, 24)).size(20)).no_bg()),
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

        if let Some(ref txt) = self.label {
            // TODO Would rotation make any sense? Or at least adjust position/size while turning.
            // Buses are a constant length, so hardcoding this is fine.
            g.draw_text_at_mapspace(txt, self.body.dist_along(Distance::meters(9.0)).0);
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
