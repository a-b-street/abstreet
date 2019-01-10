use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Polygon, Pt2D};
use sim::{CarID, CarState, DrawCarInput};

const BIKE_WIDTH: f64 = 0.8;

pub struct DrawBike {
    pub id: CarID,
    polygon: Polygon,
    // TODO the turn arrows for bikes look way wrong
    // TODO maybe also draw lookahead buffer to know what the car is considering
    stopping_buffer: Option<Polygon>,
    state: CarState,
}

impl DrawBike {
    pub fn new(input: DrawCarInput) -> DrawBike {
        let stopping_buffer = input
            .stopping_trace
            .map(|t| t.make_polygons_blindly(BIKE_WIDTH));

        DrawBike {
            id: input.id,
            polygon: input.body.make_polygons_blindly(BIKE_WIDTH),
            stopping_buffer,
            state: input.state,
        }
    }
}

impl Renderable for DrawBike {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            match self.state {
                CarState::Debug => ctx
                    .cs
                    .get_def("debug bike", Color::BLUE.alpha(0.8))
                    .shift(self.id.0),
                // TODO Hard to see on the greenish bike lanes? :P
                CarState::Moving => ctx.cs.get_def("moving bike", Color::GREEN).shift(self.id.0),
                CarState::Stuck => ctx.cs.get_def("stuck bike", Color::RED).shift(self.id.0),
                CarState::Parked => panic!("Can't have a parked bike"),
            }
        });
        g.draw_polygon(color, &self.polygon);

        if let Some(ref t) = self.stopping_buffer {
            g.draw_polygon(
                ctx.cs
                    .get_def("bike stopping buffer", Color::RED.alpha(0.7)),
                t,
            );
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }
}
