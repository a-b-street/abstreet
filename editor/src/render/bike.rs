use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Distance, Polygon};
use map_model::Map;
use sim::{CarID, CarStatus, DrawCarInput};

const BIKE_WIDTH: Distance = Distance::const_meters(0.8);

pub struct DrawBike {
    pub id: CarID,
    polygon: Polygon,
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
        let polygon = input.body.make_polygons(BIKE_WIDTH);

        let draw_default = prerender.upload_borrowed(vec![(
            match input.status {
                // TODO color.shift(input.id.0) actually looks pretty bad still
                CarStatus::Debug => cs.get_def("debug bike", Color::BLUE.alpha(0.8)),
                // TODO Hard to see on the greenish bike lanes? :P
                CarStatus::Moving => cs.get_def("moving bike", Color::GREEN),
                CarStatus::Stuck => cs.get_def("stuck bike", Color::RED),
                CarStatus::Parked => panic!("Can't have a parked bike"),
            },
            &polygon,
        )]);

        DrawBike {
            id: input.id,
            polygon,
            zorder: input.on.get_zorder(map),
            draw_default,
        }
    }
}

impl Renderable for DrawBike {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_default);
        }
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.polygon.clone()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
