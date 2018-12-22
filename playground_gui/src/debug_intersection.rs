use crate::common::{draw_polyline, SOLID_BLACK, YELLOW};
use ezgui::{Color, GfxCtx};
use geom::{PolyLine, Pt2D};

// Copied from map_model; no need to have to rebuild that crate
const LANE_THICKNESS: f64 = 2.5;

#[allow(clippy::unreadable_literal)]
pub fn run(g: &mut GfxCtx) {
    let thin = 0.25;
    let shift1_width = LANE_THICKNESS * 0.5;
    let shift2_width = LANE_THICKNESS * 1.5;

    // All the center lines are expressed as incoming to the intersection
    let shared_pt = Pt2D::new(1983.3524141911557, 1260.9463599480669);
    let diagonal_yellow = PolyLine::new(vec![
        Pt2D::new(2165.2047110114004, 1394.0800456196182),
        shared_pt,
    ]);
    let north_yellow = PolyLine::new(vec![
        Pt2D::new(1983.7494225415583, 1187.7689787235172),
        Pt2D::new(1983.562154453436, 1221.9280601888336),
        shared_pt,
    ]);
    let south_yellow = PolyLine::new(vec![
        Pt2D::new(1979.8392648173965, 1345.9215228907012),
        Pt2D::new(1981.6744921024178, 1301.599225129214),
        Pt2D::new(1983.1876182714725, 1264.9938552786543),
        shared_pt,
    ]);

    for (yellow_line, colors) in &mut vec![
        (diagonal_yellow, RelatedColors::new(1.0, 0.0, 0.0)),
        (north_yellow, RelatedColors::new(0.0, 1.0, 0.0)),
        (south_yellow, RelatedColors::new(0.0, 0.0, 1.0)),
    ] {
        let lane1_in = yellow_line.shift(shift1_width).unwrap();
        draw_lane(g, &lane1_in, colors.next().unwrap());
        let lane2_in = yellow_line.shift(shift2_width).unwrap();
        draw_lane(g, &lane2_in, colors.next().unwrap());

        let lane1_out = yellow_line.reversed().shift(shift1_width).unwrap();
        draw_lane(g, &lane1_out, colors.next().unwrap());
        let lane2_out = yellow_line.reversed().shift(shift2_width).unwrap();
        draw_lane(g, &lane2_out, colors.next().unwrap());

        draw_polyline(g, &yellow_line, thin, YELLOW);
    }
}

struct RelatedColors {
    r: f32,
    g: f32,
    b: f32,
    count: usize,
}

impl RelatedColors {
    fn new(r: f32, g: f32, b: f32) -> RelatedColors {
        RelatedColors { r, g, b, count: 10 }
    }
}

impl Iterator for RelatedColors {
    type Item = Color;

    fn next(&mut self) -> Option<Color> {
        self.count -= 2;
        let multiplier = 0.1 * (self.count as f32);
        Some(Color::rgba_f(
            self.r * multiplier,
            self.g * multiplier,
            self.b * multiplier,
            0.8,
        ))
    }
}

pub fn draw_lane(g: &mut GfxCtx, pl: &PolyLine, color: Color) {
    g.draw_polygon(color, &pl.make_polygons(LANE_THICKNESS).unwrap());

    // Debug the center points
    draw_polyline(g, pl, 0.25, SOLID_BLACK);
}
