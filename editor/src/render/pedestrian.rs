use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, Line, Pt2D};
use map_model::Map;
use sim::{DrawPedestrianInput, PedestrianID};

const RADIUS: Distance = Distance::const_meters(1.0);

pub struct DrawPedestrian {
    pub id: PedestrianID,
    circle: Circle,
    turn_arrow: Option<Line>,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawPedestrian {
    pub fn new(
        input: DrawPedestrianInput,
        map: &Map,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> DrawPedestrian {
        let turn_arrow = if let Some(t) = input.waiting_for_turn {
            // TODO this isn't quite right, but good enough for now
            let angle = map.get_t(t).angle();
            let arrow_pt = input.pos.project_away(RADIUS, angle.opposite());
            Some(Line::new(arrow_pt, input.pos))
        } else {
            None
        };

        let circle = Circle::new(input.pos, RADIUS);

        let draw_default = prerender.upload(vec![(
            if input.preparing_bike {
                cs.get_def("pedestrian preparing bike", Color::rgb(255, 0, 144))
                    .shift(input.id.0)
            } else {
                cs.get_def("pedestrian", Color::rgb_f(0.2, 0.7, 0.7))
                    .shift(input.id.0)
            },
            circle.to_polygon(),
        )]);

        DrawPedestrian {
            id: input.id,
            circle,
            turn_arrow,
            zorder: input.on.get_zorder(map),
            draw_default,
        }
    }
}

impl Renderable for DrawPedestrian {
    fn get_id(&self) -> ID {
        ID::Pedestrian(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        if let Some(color) = opts.color {
            g.draw_circle(color, &self.circle);
        } else {
            g.redraw(&self.draw_default);
        }

        // TODO tune color, sizes
        if let Some(ref a) = self.turn_arrow {
            g.draw_arrow(
                ctx.cs.get_def("pedestrian turn arrow", Color::CYAN),
                Distance::meters(0.25),
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

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
