use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, Line, Pt2D};
use map_model::Map;
use sim::{DrawPedestrianInput, PedestrianID};

const RADIUS: f64 = 1.0;

pub struct DrawPedestrian {
    pub id: PedestrianID,
    circle: Circle,
    turn_arrow: Option<Line>,
    preparing_bike: bool,
}

impl DrawPedestrian {
    pub fn new(input: DrawPedestrianInput, map: &Map) -> DrawPedestrian {
        let turn_arrow = if let Some(t) = input.waiting_for_turn {
            // TODO this isn't quite right, but good enough for now
            let angle = map.get_t(t).angle();
            let arrow_pt = input.pos.project_away(RADIUS, angle.opposite());
            Some(Line::new(arrow_pt, input.pos))
        } else {
            None
        };

        DrawPedestrian {
            id: input.id,
            circle: Circle::new(input.pos, RADIUS),
            turn_arrow,
            preparing_bike: input.preparing_bike,
        }
    }
}

impl Renderable for DrawPedestrian {
    fn get_id(&self) -> ID {
        ID::Pedestrian(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            if self.preparing_bike {
                ctx.cs
                    .get("pedestrian preparing bike", Color::rgb(255, 0, 144))
                    .shift(self.id.0)
            } else {
                ctx.cs
                    .get("pedestrian", Color::rgb_f(0.2, 0.7, 0.7))
                    .shift(self.id.0)
            }
        });
        g.draw_circle(color, &self.circle);

        // TODO tune color, sizes
        if let Some(ref a) = self.turn_arrow {
            g.draw_rounded_arrow(
                ctx.cs.get("pedestrian turn arrow", Color::CYAN),
                0.25,
                0.3,
                a,
            );
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.circle.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.circle.contains_pt(pt)
    }
}
