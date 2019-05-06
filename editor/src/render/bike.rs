use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Circle, Distance, Line, Polygon};
use map_model::{Map, LANE_THICKNESS};
use sim::{CarID, CarStatus, DrawCarInput};

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
        let mut draw_default = Vec::new();

        // TODO Share constants with DrawPedestrian
        let body_radius = LANE_THICKNESS / 4.0;
        let body_color = match input.status {
            // TODO color.shift(input.id.0) actually looks pretty bad still
            CarStatus::Debug => cs.get_def("debug bike", Color::BLUE.alpha(0.8)),
            // TODO Hard to see on the greenish bike lanes? :P
            CarStatus::Moving => cs.get_def("moving bike", Color::GREEN),
            CarStatus::Stuck => cs.get_def("stuck bike", Color::RED),
            CarStatus::Parked => panic!("Can't have a parked bike {}", input.id),
        };

        draw_default.push((
            cs.get_def("bike frame", Color::grey(0.6)),
            input.body.make_polygons(Distance::meters(0.4)),
        ));
        {
            // Handlebars
            let (pos, angle) = input.body.dist_along(0.9 * input.body.length());
            draw_default.push((
                cs.get("bike frame"),
                Line::new(
                    pos.project_away(body_radius, angle.rotate_degs(90.0)),
                    pos.project_away(body_radius, angle.rotate_degs(-90.0)),
                )
                .make_polygons(Distance::meters(0.1)),
            ));
        }

        let (pos, facing) = input.body.dist_along(0.4 * input.body.length());
        let body_circle = Circle::new(pos, body_radius);
        draw_default.push((body_color, body_circle.to_polygon()));
        draw_default.push((
            cs.get("pedestrian head"),
            Circle::new(pos, 0.5 * body_radius).to_polygon(),
        ));

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

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_circle(color, &self.body_circle);
        } else {
            g.redraw(&self.draw_default);
        }
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.body_circle.to_polygon()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
