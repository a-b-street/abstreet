use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Distance, Polygon, Pt2D};
use map_model::Map;
use sim::{CarID, CarStatus, DrawCarInput};

const BIKE_WIDTH: Distance = Distance::const_meters(0.8);

pub struct DrawBike {
    pub id: CarID,
    polygon: Polygon,
    // TODO the turn arrows for bikes look way wrong
    // TODO maybe also draw lookahead buffer to know what the car is considering
    stopping_buffer: Option<Polygon>,

    draw_default: Drawable,
}

impl DrawBike {
    pub fn new(input: DrawCarInput, prerender: &Prerender, cs: &ColorScheme) -> DrawBike {
        let stopping_buffer = input.stopping_trace.map(|t| t.make_polygons(BIKE_WIDTH));
        let polygon = input.body.make_polygons(BIKE_WIDTH);

        let draw_default = prerender.upload_borrowed(vec![(
            match input.status {
                CarStatus::Debug => cs
                    .get_def("debug bike", Color::BLUE.alpha(0.8))
                    .shift(input.id.0),
                // TODO Hard to see on the greenish bike lanes? :P
                CarStatus::Moving => cs.get_def("moving bike", Color::GREEN).shift(input.id.0),
                CarStatus::Stuck => cs.get_def("stuck bike", Color::RED).shift(input.id.0),
                CarStatus::Parked => panic!("Can't have a parked bike"),
            },
            &polygon,
        )]);

        DrawBike {
            id: input.id,
            polygon,
            stopping_buffer,
            draw_default,
        }
    }
}

impl Renderable for DrawBike {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &DrawCtx) {
        if let Some(color) = opts.color {
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_default);
        }

        if let Some(ref t) = self.stopping_buffer {
            g.draw_polygon(
                ctx.cs
                    .get_def("bike stopping buffer", Color::RED.alpha(0.7)),
                t,
            );
        }
    }

    fn get_bounds(&self, _: &Map) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        self.polygon.contains_pt(pt)
    }
}
