use crate::helpers::{rotating_color_agents, ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender};
use geom::{Circle, Distance, Line, PolyLine, Polygon};
use map_model::{Map, LANE_THICKNESS};
use sim::{CarID, DrawCarInput};

pub struct DrawBike {
    pub id: CarID,
    body_circle: Circle,
    // TODO the turn arrows for bikes look way wrong
    zorder: isize,

    draw_default: Drawable,
}

impl DrawBike {
    pub fn new(
        input: DrawCarInput,
        map: &Map,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> DrawBike {
        let mut draw_default = GeomBatch::new();

        // TODO Share constants with DrawPedestrian
        let body_radius = LANE_THICKNESS / 4.0;
        let body_color = zoomed_color_bike(&input);
        draw_default.push(
            cs.get_def("bike frame", Color::rgb(0, 128, 128)),
            input.body.make_polygons(Distance::meters(0.4)),
        );

        let (body_pos, facing) = input.body.dist_along(0.4 * input.body.length());
        let body_circle = Circle::new(body_pos, body_radius);
        draw_default.push(body_color, body_circle.to_polygon());
        draw_default.push(
            cs.get("pedestrian head"),
            Circle::new(body_pos, 0.5 * body_radius).to_polygon(),
        );

        {
            // Handlebars
            let (hand_pos, hand_angle) = input.body.dist_along(0.9 * input.body.length());
            draw_default.push(
                cs.get("bike frame"),
                Line::new(
                    hand_pos.project_away(body_radius, hand_angle.rotate_degs(90.0)),
                    hand_pos.project_away(body_radius, hand_angle.rotate_degs(-90.0)),
                )
                .make_polygons(Distance::meters(0.1)),
            );

            // Hands
            draw_default.push(
                body_color,
                Line::new(
                    body_pos.project_away(0.9 * body_radius, facing.rotate_degs(-30.0)),
                    hand_pos.project_away(0.4 * body_radius, hand_angle.rotate_degs(-90.0)),
                )
                .make_polygons(Distance::meters(0.08)),
            );
            draw_default.push(
                body_color,
                Line::new(
                    body_pos.project_away(0.9 * body_radius, facing.rotate_degs(30.0)),
                    hand_pos.project_away(0.4 * body_radius, hand_angle.rotate_degs(90.0)),
                )
                .make_polygons(Distance::meters(0.08)),
            );
        }

        if let Some(t) = input.waiting_for_turn {
            let angle = map.get_t(t).angle();
            draw_default.push(
                cs.get("turn arrow"),
                PolyLine::new(vec![
                    body_pos.project_away(body_radius / 2.0, angle.opposite()),
                    body_pos.project_away(body_radius / 2.0, angle),
                ])
                .make_arrow(Distance::meters(0.25))
                .unwrap(),
            );
        }

        DrawBike {
            id: input.id,
            body_circle,
            zorder: input.on.get_zorder(map),
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawBike {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &DrawOptions, _: &DrawCtx) {
        g.redraw(&self.draw_default);
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        // TODO ideally a thin ring
        self.body_circle.to_polygon()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

fn zoomed_color_bike(input: &DrawCarInput) -> Color {
    rotating_color_agents(input.id.0)
}
