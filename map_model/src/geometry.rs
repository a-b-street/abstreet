// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Bounds;
use Pt2D;
use aabb_quadtree::geom::{Point, Rect};
use dimensioned::si;
use graphics::math::Vec2d;
use polyline;
use std::f64;
use vecmath;

pub mod angles {
    make_units! {
        ANGLES;
        ONE: Unitless;

        base {
            RAD: Radian, "rad";
        }
        derived {}
        constants {}

        fmt = true;
    }
    pub use self::f64consts::*;
}

pub const LANE_THICKNESS: f64 = 2.5;
pub const BIG_ARROW_THICKNESS: f64 = 0.5;

use std;
pub const EPSILON_METERS: si::Meter<f64> = si::Meter {
    value_unsafe: 0.00001,
    _marker: std::marker::PhantomData,
};

pub fn thick_line_from_angle(
    thickness: f64,
    line_length: f64,
    pt: &Pt2D,
    angle: angles::Radian<f64>,
) -> Vec<Vec<Vec2d>> {
    let pt2 = Pt2D::new(
        pt.x() + line_length * angle.value_unsafe.cos(),
        pt.y() + line_length * angle.value_unsafe.sin(),
    );
    polyline::polygons_for_polyline(&vec![*pt, pt2], thickness)
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

/*pub fn interpolate_along_line((pt1, pt2): (&Pt2D, &Pt2D), factor_along: f64) -> Vec2d {
    assert!(factor_along >= 0.0 && factor_along <= 1.0);
    let x = pt1.x + factor_along * (pt2.x - pt1.x);
    let y = pt1.y + factor_along * (pt2.y - pt1.y);
    return [x, y];
}*/

// TODO borrow or copy?
pub(crate) fn euclid_dist((pt1, pt2): (Pt2D, Pt2D)) -> si::Meter<f64> {
    return ((pt1.x() - pt2.x()).powi(2) + (pt1.y() - pt2.y()).powi(2)).sqrt() * si::M;
}

pub fn line_segments_intersect((pt1, pt2): (&Vec2d, &Vec2d), (pt3, pt4): (&Vec2d, &Vec2d)) -> bool {
    // From http://bryceboe.com/2006/10/23/line-segment-intersection-algorithm/
    is_counter_clockwise(pt1, pt3, pt4) != is_counter_clockwise(pt2, pt3, pt4)
        && is_counter_clockwise(pt1, pt2, pt3) != is_counter_clockwise(pt1, pt2, pt4)
}

fn is_counter_clockwise(pt1: &Vec2d, pt2: &Vec2d, pt3: &Vec2d) -> bool {
    (pt3[1] - pt1[1]) * (pt2[0] - pt1[0]) > (pt2[1] - pt1[1]) * (pt3[0] - pt1[0])
}

pub fn line_segment_intersection(l1: (Pt2D, Pt2D), l2: (Pt2D, Pt2D)) -> Option<Pt2D> {
    // TODO shoddy way of implementing this
    // TODO doesn't handle nearly parallel lines
    if !line_segments_intersect(
        (&l1.0.to_vec(), &l1.1.to_vec()),
        (&l2.0.to_vec(), &l2.1.to_vec()),
    ) {
        return None;
    }
    polyline::line_intersection(l1, l2)
}

pub fn dist_along_line((pt1, pt2): (&Pt2D, &Pt2D), dist_along: f64) -> Vec2d {
    //assert!(euclid_dist(&pt1, &pt2) <= dist_along);
    let vec = vecmath::vec2_normalized([pt2.x() - pt1.x(), pt2.y() - pt1.y()]);
    [pt1.x() + dist_along * vec[0], pt1.y() + dist_along * vec[1]]
}

// TODO rm the other one
pub fn safe_dist_along_line((pt1, pt2): (&Pt2D, &Pt2D), dist_along: si::Meter<f64>) -> Vec2d {
    let len = euclid_dist((*pt1, *pt2));
    if dist_along > len + EPSILON_METERS {
        panic!("cant do {} along a line of length {}", dist_along, len);
    }

    let percent = (dist_along / len).value_unsafe;
    [
        pt1.x() + percent * (pt2.x() - pt1.x()),
        pt1.y() + percent * (pt2.y() - pt1.y()),
    ]
    // TODO unit test
    /*
    let res_len = euclid_dist((pt1, &Pt2D::new(res[0], res[1])));
    if res_len != dist_along {
        println!("whats the delta btwn {} and {}?", res_len, dist_along);
    }
    */}

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

pub fn gps_to_screen_space(gps: &Pt2D, b: &Bounds) -> Pt2D {
    // If not, havoc ensues
    assert!(b.contains(gps.x(), gps.y()));

    // Invert y, so that the northernmost latitude is 0. Screen drawing order, not Cartesian grid.
    let base = Pt2D::new(b.min_x, b.max_y);
    // Apparently the aabb_quadtree can't handle 0, so add a bit.
    // TODO epsilon or epsilon - 1.0?
    let dx = base.gps_dist_meters(&Pt2D::new(gps.x(), base.y())) + f64::EPSILON;
    let dy = base.gps_dist_meters(&Pt2D::new(base.x(), gps.y())) + f64::EPSILON;
    // By default, 1 meter is one pixel. Normal zooming can change that. If we did scaling here,
    // then we'd have to update all of the other constants too.
    Pt2D::new(dx, dy)
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

pub fn angle(from: &Pt2D, to: &Pt2D) -> angles::Radian<f64> {
    // DON'T invert y here
    let mut theta = (to.y() - from.y()).atan2(to.x() - from.x());
    // Normalize for easy output
    if theta < 0.0 {
        theta += 2.0 * f64::consts::PI;
    }
    theta * angles::RAD
}
