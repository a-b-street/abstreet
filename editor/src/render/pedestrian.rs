use aabb_quadtree::geom::Rect;
use colors::Colors;
use ezgui::{shift_color, GfxCtx};
use geom::{Circle, Line, Pt2D};
use map_model::Map;
use objects::{Ctx, ID};
use render::{get_bbox, RenderOptions, Renderable};
use sim::{DrawPedestrianInput, PedestrianID};

const RADIUS: f64 = 1.0;

pub struct DrawPedestrian {
    pub id: PedestrianID,
    circle: Circle,
    turn_arrow: Option<Line>,
}

impl DrawPedestrian {
    pub fn new(input: DrawPedestrianInput, map: &Map) -> DrawPedestrian {
        let turn_arrow = if let Some(t) = input.waiting_for_turn {
            // TODO this isn't quite right, but good enough for now
            let angle = map.get_t(t).line.angle();
            let arrow_pt = input.pos.project_away(RADIUS, angle.opposite());
            Some(Line::new(arrow_pt, input.pos))
        } else {
            None
        };

        DrawPedestrian {
            id: input.id,
            circle: Circle::new(input.pos, RADIUS),
            turn_arrow,
        }
    }
}

impl Renderable for DrawPedestrian {
    fn get_id(&self) -> ID {
        ID::Pedestrian(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        let color = opts
            .color
            .unwrap_or(shift_color(ctx.cs.get(Colors::Pedestrian), self.id.0));
        g.draw_circle(color, &self.circle);

        // TODO tune color, sizes
        if let Some(ref a) = self.turn_arrow {
            g.draw_rounded_arrow([0.0, 1.0, 1.0, 1.0], 0.25, 0.3, a);
        }
    }

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.circle.get_bounds())
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.circle.contains_pt(pt)
    }

    fn tooltip_lines(&self, _map: &Map) -> Vec<String> {
        vec![self.id.to_string()]
    }
}
