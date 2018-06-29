// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::{Point, Rect};
use geom::{Angle, Bounds, PolyLine, Pt2D};
use graphics::math::Vec2d;
use std::f64;

pub const LANE_THICKNESS: f64 = 2.5;
pub const BIG_ARROW_THICKNESS: f64 = 0.5;

pub fn thick_line_from_angle(
    thickness: f64,
    line_length: f64,
    pt: &Pt2D,
    angle: Angle,
) -> Vec<Vec<Vec2d>> {
    let pt2 = pt.project_away(line_length, angle);
    // Shouldn't ever fail for a single line
    PolyLine::new(vec![*pt, pt2])
        .make_polygons(thickness)
        .unwrap()
}

// Algorithm from https://wrf.ecse.rpi.edu//Research/Short_Notes/pnpoly.html
pub fn point_in_polygon(x: f64, y: f64, polygon: &[Vec2d]) -> bool {
    // TODO fix map conversion
    //assert_eq!(polygon[0], polygon[polygon.len() - 1]);
    if polygon[0] != polygon[polygon.len() - 1] {
        println!("WARNING: polygon {:?} isn't closed", polygon);
        return false;
    }

    let mut inside = false;
    for (pt1, pt2) in polygon.iter().zip(polygon.iter().skip(1)) {
        let x1 = pt1[0];
        let y1 = pt1[1];
        let x2 = pt2[0];
        let y2 = pt2[1];
        let intersect = ((y1 > y) != (y2 > y)) && (x < (x2 - x1) * (y - y1) / (y2 - y1) + x1);
        if intersect {
            inside = !inside;
        }
    }
    inside
}

pub fn point_in_circle(x: f64, y: f64, center: Vec2d, radius: f64) -> bool {
    // avoid sqrt by squaring radius instead
    (x - center[0]).powi(2) + (y - center[1]).powi(2) < radius.powi(2)
}

pub fn get_bbox_for_polygons(polygons: &[Vec<Vec2d>]) -> Rect {
    let mut b = Bounds::new();
    for poly in polygons {
        for pt in poly {
            b.update(pt[0], pt[1]);
        }
    }
    Rect {
        top_left: Point {
            x: b.min_x as f32,
            y: b.min_y as f32,
        },
        bottom_right: Point {
            x: b.max_x as f32,
            y: b.max_y as f32,
        },
    }
}

pub fn circle(center_x: f64, center_y: f64, radius: f64) -> [f64; 4] {
    [
        center_x - radius,
        center_y - radius,
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
