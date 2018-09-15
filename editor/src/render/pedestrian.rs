use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use geom::Pt2D;
use graphics;
use map_model::{geometry, Map};
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable};
use sim::{DrawPedestrianInput, PedestrianID};

const RADIUS: f64 = 1.0;

// TODO should this live in editor/render?
pub struct DrawPedestrian {
    pub id: PedestrianID,
    circle: [f64; 4],
    turn_arrow: Option<[f64; 4]>,
}

impl DrawPedestrian {
    pub fn new(input: DrawPedestrianInput, map: &Map) -> DrawPedestrian {
        let turn_arrow = if let Some(t) = input.waiting_for_turn {
            // TODO this isn't quite right, but good enough for now
            let angle = map.get_t(t).line.angle();
            let arrow_pt = input.pos.project_away(RADIUS, angle.opposite());
            Some([arrow_pt.x(), arrow_pt.y(), input.pos.x(), input.pos.y()])
        } else {
            None
        };

        DrawPedestrian {
            id: input.id,
            circle: geometry::make_circle(input.pos, RADIUS),
            turn_arrow,
        }
    }
}

impl Renderable for DrawPedestrian {
    fn get_id(&self) -> ID {
        ID::Pedestrian(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, _ctx: Ctx) {
        g.draw_ellipse(opts.color, self.circle);

        // TODO tune color, sizes
        if let Some(a) = self.turn_arrow {
            g.draw_arrow(
                &graphics::Line::new_round([0.0, 1.0, 1.0, 1.0], 0.25),
                a,
                0.3,
            );
        }
    }

    fn get_bbox(&self) -> Rect {
        geometry::circle_to_bbox(&self.circle)
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        geometry::point_in_circle(&self.circle, pt)
    }

    fn tooltip_lines(&self, _map: &Map) -> Vec<String> {
        vec![self.id.to_string()]
    }
}
