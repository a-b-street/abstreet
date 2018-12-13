use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, PolyLine, Polygon, Pt2D};
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
            polygon: thick_line_from_angle(
                BIKE_WIDTH,
                input.vehicle_length.value_unsafe,
                input.front,
                // find the back of the bike relative to the front
                input.angle.opposite(),
            ),
            stopping_buffer,
            state: input.state,
        }
    }
}

impl Renderable for DrawBike {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &mut Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            match self.state {
                CarState::Debug => ctx
                    .cs
                    .get_def("debug bike", Color::rgba(0, 0, 255, 0.8))
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
                    .get_def("bike stopping buffer", Color::rgba(255, 0, 0, 0.7)),
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

fn thick_line_from_angle(thickness: f64, line_length: f64, pt: Pt2D, angle: Angle) -> Polygon {
    let pt2 = pt.project_away(line_length, angle);
    // Shouldn't ever fail for a single line
    PolyLine::new(vec![pt, pt2]).make_polygons_blindly(thickness)
}
