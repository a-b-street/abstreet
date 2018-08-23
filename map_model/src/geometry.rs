// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::{Point, Rect};
use geom::{Angle, PolyLine, Polygon, Pt2D};
use std::f64;

pub const LANE_THICKNESS: f64 = 2.5;
pub const BIG_ARROW_THICKNESS: f64 = 0.5;

pub fn thick_line_from_angle(thickness: f64, line_length: f64, pt: Pt2D, angle: Angle) -> Polygon {
    let pt2 = pt.project_away(line_length, angle);
    // Shouldn't ever fail for a single line
    PolyLine::new(vec![pt, pt2]).make_polygons_blindly(thickness)
}

pub fn center_of_circle(c: &[f64; 4]) -> Pt2D {
    let radius = c[2] / 2.0;
    Pt2D::new(c[0] + radius, c[1] + radius)
}

pub fn point_in_circle(c: &[f64; 4], pt: Pt2D) -> bool {
    let radius = c[2] / 2.0;
    let center = center_of_circle(c);
    // avoid sqrt by squaring radius instead
    (pt.x() - center.x()).powi(2) + (pt.y() - center.y()).powi(2) < radius.powi(2)
}

pub fn make_circle(center: Pt2D, radius: f64) -> [f64; 4] {
    [
        center.x() - radius,
        center.y() - radius,
        2.0 * radius,
        2.0 * radius,
    ]
}

pub fn circle_to_bbox(c: &[f64; 4]) -> Rect {
    Rect {
        top_left: Point {
            x: c[0] as f32,
            y: c[1] as f32,
        },
        bottom_right: Point {
            x: (c[0] + c[2]) as f32,
            y: (c[1] + c[3]) as f32,
        },
    }
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
