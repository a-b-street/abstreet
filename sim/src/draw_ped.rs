use ezgui::GfxCtx;
use geom::Pt2D;
use graphics;
use map_model::geometry;
use PedestrianID;

const RADIUS: f64 = 1.0;

// TODO should this live in editor/render?
// TODO show turns waited for
pub struct DrawPedestrian {
    pub id: PedestrianID,
    circle: [f64; 4],
}

impl DrawPedestrian {
    pub(crate) fn new(id: PedestrianID, pos: Pt2D) -> DrawPedestrian {
        DrawPedestrian {
            id,
            circle: geometry::circle(pos.x(), pos.y(), RADIUS),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        g.draw_ellipse(color, self.circle);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_circle(x, y, [self.circle[0], self.circle[1]], RADIUS)
    }
}
