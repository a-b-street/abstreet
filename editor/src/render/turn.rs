use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, Line, Pt2D};
use map_model::{Map, Turn, TurnID, LANE_THICKNESS};
use objects::{Ctx, ID};
use render::{
    RenderOptions, Renderable, BIG_ARROW_THICKNESS, BIG_ARROW_TIP_LENGTH, CROSSWALK_LINE_THICKNESS,
    TURN_ICON_ARROW_LENGTH, TURN_ICON_ARROW_THICKNESS, TURN_ICON_ARROW_TIP_LENGTH,
};
use std::f64;

#[derive(Debug)]
pub struct DrawTurn {
    pub id: TurnID,
    icon_circle: Circle,
    icon_arrow: Line,
}

impl DrawTurn {
    pub fn new(map: &Map, turn: &Turn, offset_along_lane: usize) -> DrawTurn {
        let offset_along_lane = offset_along_lane as f64;
        let angle = turn.angle();

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
            icon_circle,
            icon_arrow,
        }
    }

    pub fn draw_full(t: &Turn, g: &mut GfxCtx, color: Color) {
        g.draw_polygon(
            color,
            &t.geom.make_polygons(2.0 * BIG_ARROW_THICKNESS).unwrap(),
        );
        // And a cap on the arrow
        g.draw_rounded_arrow(
            color,
            BIG_ARROW_THICKNESS,
            BIG_ARROW_TIP_LENGTH,
            &t.geom.last_line(),
        );
    }
}

// Little weird, but this is focused on the turn icon, not the full visualization
impl Renderable for DrawTurn {
    fn get_id(&self) -> ID {
        ID::Turn(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        // Some plugins hide icons entirely.
        if ctx.hints.hide_turn_icons.contains(&self.id) {
            return;
        }

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

#[derive(Debug)]
pub struct DrawCrosswalk {
    pub id1: TurnID,
    pub id2: TurnID,
    lines: Vec<Line>,
}

impl DrawCrosswalk {
    pub fn new(turn: &Turn) -> DrawCrosswalk {
        let mut lines = Vec::new();
        // Start at least LANE_THICKNESS out to not hit sidewalk corners. Also account for
        // the thickness of the crosswalk line itself. Center the lines inside these two
        // boundaries.
        let boundary = (LANE_THICKNESS + CROSSWALK_LINE_THICKNESS) * si::M;
        let tile_every = 0.6 * LANE_THICKNESS * si::M;
        let line = {
            // The middle line in the crosswalk geometry is the main crossing line.
            let pts = turn.geom.points();
            Line::new(pts[1], pts[2])
        };

        let available_length = line.length() - (2.0 * boundary);
        if available_length > 0.0 * si::M {
            let num_markings = (available_length / tile_every).floor() as usize;
            let mut dist_along =
                boundary + (available_length - tile_every * (num_markings as f64)) / 2.0;
            // TODO Seems to be an off-by-one sometimes. Not enough of these.
            for _ in 0..=num_markings {
                let pt1 = line.dist_along(dist_along);
                // Reuse perp_line. Project away an arbitrary amount
                let pt2 = pt1.project_away(1.0, turn.angle());
                lines.push(perp_line(Line::new(pt1, pt2), LANE_THICKNESS));
                dist_along += tile_every;
            }
        }

        DrawCrosswalk {
            id1: turn.id,
            id2: turn.other_crosswalk_id(),
            lines,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        for line in &self.lines {
            g.draw_line(color, CROSSWALK_LINE_THICKNESS, line);
        }
    }
}

// TODO copied from DrawLane
fn perp_line(l: Line, length: f64) -> Line {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    Line::new(pt1, pt2)
}
