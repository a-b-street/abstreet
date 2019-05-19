use crate::helpers::ColorScheme;
use crate::render::{
    BIG_ARROW_THICKNESS, CROSSWALK_LINE_THICKNESS, TURN_ICON_ARROW_LENGTH,
    TURN_ICON_ARROW_THICKNESS,
};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{Map, Turn, TurnID, LANE_THICKNESS};

pub struct DrawTurn {
    pub id: TurnID,
    icon_circle: Circle,
    icon_arrow: Vec<Polygon>,
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
        let icon_arrow = Line::new(icon_src, icon_dst)
            .to_polyline()
            .make_arrow(TURN_ICON_ARROW_THICKNESS)
            .unwrap();

        DrawTurn {
            id: turn.id,
            icon_circle,
            icon_arrow,
        }
    }

    pub fn full_geom(t: &Turn, batch: &mut GeomBatch, color: Color) {
        batch.extend(color, t.geom.make_arrow(BIG_ARROW_THICKNESS * 2.0).unwrap());
    }

    pub fn draw_full(t: &Turn, g: &mut GfxCtx, color: Color) {
        let mut batch = GeomBatch::new();
        DrawTurn::full_geom(t, &mut batch, color);
        batch.draw(g);

        // For debugging
        /*for pt in t.geom.points() {
            g.draw_circle(Color::RED, &geom::Circle::new(*pt, Distance::meters(0.4)));
        }*/
    }

    pub fn draw_dashed(turn: &Turn, batch: &mut GeomBatch, color: Color) {
        let dash_len = Distance::meters(1.0);
        batch.extend(
            color,
            turn.geom
                .dashed_polygons(BIG_ARROW_THICKNESS, dash_len, Distance::meters(0.5)),
        );
        // And a cap on the arrow. In case the last line is long, trim it to be the dash
        // length.
        let last_line = turn.geom.last_line();
        let last_len = last_line.length();
        let arrow_line = if last_len <= dash_len {
            last_line
        } else {
            Line::new(last_line.dist_along(last_len - dash_len), last_line.pt2())
        };
        batch.extend(
            color,
            arrow_line
                .to_polyline()
                .make_arrow(BIG_ARROW_THICKNESS)
                .unwrap(),
        );
    }

    pub fn outline_geom(turn: &Turn, batch: &mut GeomBatch, color: Color) {
        batch.extend(
            color,
            turn.geom
                .make_arrow_outline(BIG_ARROW_THICKNESS * 2.0, BIG_ARROW_THICKNESS / 2.0)
                .unwrap(),
        );
    }

    pub fn draw_icon(
        &self,
        batch: &mut GeomBatch,
        cs: &ColorScheme,
        arrow_color: Color,
        selected: bool,
    ) {
        batch.push(
            if selected {
                cs.get("selected")
            } else {
                cs.get_def("turn icon circle", Color::grey(0.6))
            },
            self.icon_circle.to_polygon(),
        );
        batch.extend(arrow_color, self.icon_arrow.clone());
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
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

        let mut draw = GeomBatch::new();
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
                draw.push(
                    cs.get_def("crosswalk", Color::WHITE),
                    perp_line(Line::new(pt1, pt2), LANE_THICKNESS)
                        .make_polygons(CROSSWALK_LINE_THICKNESS),
                );
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
