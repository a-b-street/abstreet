use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, Line, PolyLine, Pt2D};
use map_model::{Map, Turn, TurnID, TurnType};
use objects::{Ctx, ID};
use render::{
    RenderOptions, Renderable, BIG_ARROW_THICKNESS, BIG_ARROW_TIP_LENGTH, TURN_ICON_ARROW_LENGTH,
    TURN_ICON_ARROW_THICKNESS, TURN_ICON_ARROW_TIP_LENGTH,
};
use std::f64;

#[derive(Debug)]
pub struct DrawTurn {
    pub id: TurnID,
    line: Line,
    icon_circle: Circle,
    icon_arrow: Line,
}

impl DrawTurn {
    pub fn new(map: &Map, turn: &Turn, offset_along_lane: usize) -> DrawTurn {
        let offset_along_lane = offset_along_lane as f64;
        let angle = turn.line.angle();

        let end_line = map.get_l(turn.id.src).end_line(turn.id.parent);
        // Start the distance from the intersection
        let icon_center = end_line
            .reverse()
            .unbounded_dist_along((offset_along_lane + 0.5) * TURN_ICON_ARROW_LENGTH * si::M);

        let icon_circle = Circle::new(icon_center, TURN_ICON_ARROW_LENGTH / 2.0);

        let icon_src = icon_center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite());
        let icon_dst = icon_center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle);
        let icon_arrow = Line::new(icon_src, icon_dst);

        DrawTurn {
            id: turn.id,
            line: turn.line.clone(),
            icon_circle,
            icon_arrow,
        }
    }

    pub fn draw_full(&self, map: &Map, g: &mut GfxCtx, color: Color) {
        match map.get_t(self.id).turn_type {
            TurnType::Left | TurnType::Right => {
                use nbez::{Bez3o, BezCurve, Point2d};

                fn to_pt(pt: Pt2D) -> Point2d<f64> {
                    Point2d::new(pt.x(), pt.y())
                }
                fn from_pt(pt: Point2d<f64>) -> Pt2D {
                    Pt2D::new(pt.x, pt.y)
                }

                // The control points are straight out/in from the source/destination lanes, so
                // that the car exits and enters at the same angle as the road.
                let src_line = map.get_l(self.id.src).last_line();
                let dst_line = map.get_l(self.id.dst).first_line().reverse();

                let curve = Bez3o::new(
                    to_pt(self.line.pt1()),
                    to_pt(src_line.unbounded_dist_along(src_line.length() + 5.0 * si::M)),
                    to_pt(dst_line.unbounded_dist_along(dst_line.length() + 5.0 * si::M)),
                    to_pt(self.line.pt2()),
                );
                let pieces = 5;
                let polyline = PolyLine::new(
                    (0..=pieces)
                        .map(|i| from_pt(curve.interp(1.0 / (pieces as f64) * (i as f64)).unwrap()))
                        .collect(),
                );
                g.draw_polygon(
                    color,
                    &polyline.make_polygons(2.0 * BIG_ARROW_THICKNESS).unwrap(),
                );

                // And a cap on the arrow
                g.draw_rounded_arrow(
                    color,
                    BIG_ARROW_THICKNESS,
                    BIG_ARROW_TIP_LENGTH,
                    &polyline.last_line(),
                );
            }
            _ => {
                g.draw_rounded_arrow(color, BIG_ARROW_THICKNESS, BIG_ARROW_TIP_LENGTH, &self.line);
            }
        };
    }
}

// Little weird, but this is focused on the turn icon, not the full visualization
impl Renderable for DrawTurn {
    fn get_id(&self) -> ID {
        ID::Turn(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        g.draw_circle(
            ctx.cs.get("turn icon circle", Color::grey(0.3)),
            &self.icon_circle,
        );

        g.draw_arrow(
            opts.color
                .unwrap_or_else(|| ctx.cs.get("inactive turn icon", Color::grey(0.7))),
            TURN_ICON_ARROW_THICKNESS,
            TURN_ICON_ARROW_TIP_LENGTH,
            &self.icon_arrow,
        );
    }

    fn get_bounds(&self) -> Bounds {
        self.icon_circle.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.icon_circle.contains_pt(pt)
    }
}
