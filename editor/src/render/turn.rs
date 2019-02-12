use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, ID};
use crate::render::{
    RenderOptions, Renderable, BIG_ARROW_THICKNESS, CROSSWALK_LINE_THICKNESS,
    TURN_ICON_ARROW_LENGTH, TURN_ICON_ARROW_THICKNESS,
};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, Line, Pt2D};
use map_model::{Map, Turn, TurnID, LANE_THICKNESS};

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
            .unbounded_dist_along(TURN_ICON_ARROW_LENGTH * (offset_along_lane + 0.5));

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
        // TODO This is hiding a real problem... some composite turns probably need to have their
        // geometry simplified a bit.
        if let Some(pl) = t.geom.without_last_line() {
            g.draw_polygon(color, &pl.make_polygons(BIG_ARROW_THICKNESS * 2.0));
        }
        // And a cap on the arrow
        g.draw_arrow(color, BIG_ARROW_THICKNESS * 2.0, &t.geom.last_line());
    }

    pub fn draw_dashed(turn: &Turn, g: &mut GfxCtx, color: Color) {
        let dash_len = Distance::meters(1.0);
        let dashed =
            turn.geom
                .dashed_polygons(BIG_ARROW_THICKNESS, dash_len, Distance::meters(0.5));
        g.draw_polygon_batch(dashed.iter().map(|poly| (color, poly)).collect());
        // And a cap on the arrow. In case the last line is long, trim it to be the dash
        // length.
        let last_line = turn.geom.last_line();
        let last_len = last_line.length();
        let arrow_line = if last_len <= dash_len {
            last_line
        } else {
            Line::new(last_line.dist_along(last_len - dash_len), last_line.pt2())
        };
        g.draw_arrow(color, BIG_ARROW_THICKNESS, &arrow_line);
    }
}

// Little weird, but this is focused on the turn icon, not the full visualization
impl Renderable for DrawTurn {
    fn get_id(&self) -> ID {
        ID::Turn(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &DrawCtx) {
        // Some plugins hide icons entirely.
        if ctx.hints.hide_turn_icons.contains(&self.id) {
            return;
        }

        g.draw_circle(
            if opts.is_selected {
                ctx.cs.get("selected")
            } else {
                ctx.cs.get_def("turn icon circle", Color::grey(0.3))
            },
            &self.icon_circle,
        );

        g.draw_arrow(
            opts.color
                .unwrap_or_else(|| ctx.cs.get_def("inactive turn icon", Color::grey(0.7))),
            TURN_ICON_ARROW_THICKNESS,
            &self.icon_arrow,
        );
    }

    fn get_bounds(&self, _: &Map) -> Bounds {
        self.icon_circle.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        self.icon_circle.contains_pt(pt)
    }
}

pub struct DrawCrosswalk {
    // This is arbitrarily one of the two IDs
    pub id1: TurnID,
    draw_default: Drawable,
}

impl DrawCrosswalk {
    pub fn new(turn: &Turn, prerender: &Prerender, cs: &ColorScheme) -> DrawCrosswalk {
        // Start at least LANE_THICKNESS out to not hit sidewalk corners. Also account for
        // the thickness of the crosswalk line itself. Center the lines inside these two
        // boundaries.
        let boundary = LANE_THICKNESS + CROSSWALK_LINE_THICKNESS;
        let tile_every = LANE_THICKNESS * 0.6;
        let line = {
            // The middle line in the crosswalk geometry is the main crossing line.
            let pts = turn.geom.points();
            Line::new(pts[1], pts[2])
        };

        let mut draw = Vec::new();
        let available_length = line.length() - (boundary * 2.0);
        if available_length > Distance::ZERO {
            let num_markings = (available_length / tile_every).floor() as usize;
            let mut dist_along =
                boundary + (available_length - tile_every * (num_markings as f64)) / 2.0;
            // TODO Seems to be an off-by-one sometimes. Not enough of these.
            for _ in 0..=num_markings {
                let pt1 = line.dist_along(dist_along);
                // Reuse perp_line. Project away an arbitrary amount
                let pt2 = pt1.project_away(Distance::meters(1.0), turn.angle());
                draw.push((
                    cs.get_def("crosswalk", Color::WHITE),
                    perp_line(Line::new(pt1, pt2), LANE_THICKNESS)
                        .make_polygons(CROSSWALK_LINE_THICKNESS),
                ));
                dist_along += tile_every;
            }
        }

        DrawCrosswalk {
            id1: turn.id,
            draw_default: prerender.upload(draw),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw_default);
    }
}

// TODO copied from DrawLane
fn perp_line(l: Line, length: Distance) -> Line {
    let pt1 = l.shift_right(length / 2.0).pt1();
    let pt2 = l.shift_left(length / 2.0).pt1();
    Line::new(pt1, pt2)
}
