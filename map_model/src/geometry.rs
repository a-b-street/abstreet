// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use geom::{Angle, PolyLine, Polygon, Pt2D};
use std::f64;

pub const LANE_THICKNESS: f64 = 2.5;
pub const BIG_ARROW_THICKNESS: f64 = 0.5;

pub fn thick_line_from_angle(thickness: f64, line_length: f64, pt: Pt2D, angle: Angle) -> Polygon {
    let pt2 = pt.project_away(line_length, angle);
    // Shouldn't ever fail for a single line
    PolyLine::new(vec![pt, pt2]).make_polygons_blindly(thickness)
}

pub fn center(pts: &Vec<Pt2D>) -> Pt2D {
    let mut x = 0.0;
    let mut y = 0.0;
    for pt in pts {
        x += pt.x();
        y += pt.y();
    }
    let len = pts.len() as f64;
    Pt2D::new(x / len, y / len)
}

pub fn regular_polygon(center: Pt2D, sides: usize, length: f64) -> Polygon {
    let mut pts = Vec::new();
    for i in 0..sides {
        let theta = (i as f64) * 2.0 * f64::consts::PI / (sides as f64);
        pts.push(Pt2D::new(
            length * theta.cos() + center.x(),
            length * theta.sin() + center.y(),
        ));
    }
    let first_pt = pts[0];
    pts.push(first_pt);
    Polygon::new(&pts)
}
