use ezgui::GfxCtx;
use geom::Pt2D;
use graphics;
use map_model::{geometry, Turn};
use PedestrianID;

const RADIUS: f64 = 1.0;

// TODO should this live in editor/render?
pub struct DrawPedestrian {
    pub id: PedestrianID,
    circle: [f64; 4],
    turn_arrow: Option<[f64; 4]>,
}

impl DrawPedestrian {
    pub fn new(id: PedestrianID, pos: Pt2D, waiting_for_turn: Option<&Turn>) -> DrawPedestrian {
        let turn_arrow = if let Some(t) = waiting_for_turn {
            // TODO this isn't quite right, but good enough for now
            let angle = t.line.angle();
            let arrow_pt = pos.project_away(RADIUS, angle.opposite());
            Some([arrow_pt.x(), arrow_pt.y(), pos.x(), pos.y()])
        } else {
            None
        };

        DrawPedestrian {
            id,
            circle: geometry::make_circle(pos, RADIUS),
            turn_arrow,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        g.draw_ellipse(color, self.circle);

        // TODO tune color, sizes
        if let Some(a) = self.turn_arrow {
            g.draw_arrow(
                &graphics::Line::new_round([0.0, 1.0, 1.0, 1.0], 0.25),
                a,
                0.3,
            );
        }
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        geometry::point_in_circle(&self.circle, pt)
    }

    pub fn focus_pt(&self) -> Pt2D {
        geometry::center_of_circle(&self.circle)
    }
}
