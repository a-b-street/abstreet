use geom::{ArrowCap, Circle, Distance, Line, PolyLine, Polygon};
use map_model::{Map, SIDEWALK_THICKNESS};
use sim::{CarID, DrawCarInput};
use widgetry::{Drawable, GeomBatch, GfxCtx, Prerender};

use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable};
use crate::AppLike;

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
        let body_radius = SIDEWALK_THICKNESS / 4.0;
        let body_color = cs.rotating_color_agents(input.id.0);
        draw_default.push(
            cs.bike_frame,
            input.body.make_polygons(Distance::meters(0.4)),
        );

        let err = format!("{} on {} has weird body", input.id, input.on);
        let (body_pos, facing) = input
            .body
            .dist_along(0.4 * input.body.length())
            .expect(&err);
        let body_circle = Circle::new(body_pos, body_radius);
        draw_default.push(body_color, body_circle.to_polygon());
        draw_default.push(
            cs.ped_head,
            Circle::new(body_pos, 0.5 * body_radius).to_polygon(),
        );

        {
            // Handlebars
            let (hand_pos, hand_angle) = input
                .body
                .dist_along(0.9 * input.body.length())
                .expect(&err);
            draw_default.push(
                cs.bike_frame,
                Line::new(
                    hand_pos.project_away(body_radius, hand_angle.rotate_degs(90.0)),
                    hand_pos.project_away(body_radius, hand_angle.rotate_degs(-90.0)),
                )
                .unwrap()
                .make_polygons(Distance::meters(0.1)),
            );

            // Hands
            draw_default.push(
                body_color,
                Line::new(
                    body_pos.project_away(0.9 * body_radius, facing.rotate_degs(-30.0)),
                    hand_pos.project_away(0.4 * body_radius, hand_angle.rotate_degs(-90.0)),
                )
                .unwrap()
                .make_polygons(Distance::meters(0.08)),
            );
            draw_default.push(
                body_color,
                Line::new(
                    body_pos.project_away(0.9 * body_radius, facing.rotate_degs(30.0)),
                    hand_pos.project_away(0.4 * body_radius, hand_angle.rotate_degs(90.0)),
                )
                .unwrap()
                .make_polygons(Distance::meters(0.08)),
            );
        }

        if let Some(t) = input.waiting_for_turn {
            let angle = map.get_t(t).angle();
            draw_default.push(
                cs.turn_arrow,
                PolyLine::must_new(vec![
                    body_pos.project_away(body_radius / 2.0, angle.opposite()),
                    body_pos.project_away(body_radius / 2.0, angle),
                ])
                .make_arrow(Distance::meters(0.15), ArrowCap::Triangle),
            );
        }

        let zorder = input
            .partly_on
            .into_iter()
            .chain(vec![input.on])
            .map(|on| on.get_zorder(map))
            .max()
            .unwrap();
        DrawBike {
            id: input.id,
            body_circle,
            zorder,
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawBike {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &dyn AppLike, _: &DrawOptions) {
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
