use geom::{ArrowCap, Circle, Distance, Line, PolyLine, Polygon, Pt2D};
use map_model::{Map, SIDEWALK_THICKNESS};
use sim::{CarID, DrawCarInput, Intent, Sim};
use widgetry::{Drawable, GeomBatch, GfxCtx, Prerender};

use crate::colors::ColorScheme;
use crate::render::{grey_out_unhighlighted_people, DrawOptions, Renderable, OUTLINE_THICKNESS};
use crate::{AppLike, ID};

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
        sim: &Sim,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> DrawBike {
        let mut draw_default = GeomBatch::new();

        // TODO Share constants with DrawPedestrian
        let body_radius = SIDEWALK_THICKNESS / 4.0;
        let body_color = grey_out_unhighlighted_people(
            cs.rotating_color_agents(input.id.id),
            &input.person,
            sim,
        );
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
                    body_pos.project_away(body_radius / 2.0, (facing + angle).opposite()),
                    body_pos.project_away(body_radius / 2.0, facing + angle),
                ])
                .make_arrow(Distance::meters(0.15), ArrowCap::Triangle),
            );
        }

        if input.intent == Some(Intent::SteepUphill) {
            let bubble_z = -0.0001;
            let mut bubble_batch =
                GeomBatch::load_svg(prerender, "system/assets/map/thought_bubble.svg")
                    .scale(0.05)
                    .centered_on(input.body.middle())
                    .translate(2.0, -3.5)
                    .set_z_offset(bubble_z);
            bubble_batch.append(
                GeomBatch::load_svg(prerender, "system/assets/tools/uphill.svg")
                    .scale(0.05)
                    .centered_on(input.body.middle())
                    .translate(2.2, -4.2)
                    .set_z_offset(bubble_z),
            );

            draw_default.append(bubble_batch);
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
        Circle::new(self.body_circle.center, Distance::meters(2.0))
            .to_outline(OUTLINE_THICKNESS)
            .unwrap()
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        Circle::new(self.body_circle.center, Distance::meters(2.0)).contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
