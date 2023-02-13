use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{Angle, Distance, PolyLine, Polygon, Pt2D, EPSILON_DIST};

/// A line segment.
// TODO Rename?
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Line(Pt2D, Pt2D);

impl Line {
    /// Creates a line segment between two points, which must not be the same
    pub fn new(pt1: Pt2D, pt2: Pt2D) -> Result<Line> {
        if pt1.dist_to(pt2) <= EPSILON_DIST {
            bail!("Line from {:?} to {:?} too small", pt1, pt2);
        }
        Ok(Line(pt1, pt2))
    }

    /// Equivalent to `Line::new(pt1, pt2).unwrap()`. Use this to effectively document an assertion
    /// at the call-site.
    pub fn must_new(pt1: Pt2D, pt2: Pt2D) -> Line {
        Line::new(pt1, pt2).unwrap()
    }

    /// Returns an infinite line passing through this line's two points.
    pub fn infinite(&self) -> InfiniteLine {
        InfiniteLine(self.0, self.1)
    }

    /// Returns the first point in this line segment.
    pub fn pt1(&self) -> Pt2D {
        self.0
    }

    /// Returns the second point in this line segment.
    pub fn pt2(&self) -> Pt2D {
        self.1
    }

    /// Returns the two points in this line segment.
    pub fn points(&self) -> Vec<Pt2D> {
        vec![self.0, self.1]
    }

    /// Returns a polyline containing these two points.
    pub fn to_polyline(&self) -> PolyLine {
        PolyLine::must_new(self.points())
    }

    /// Returns a thick line segment.
    pub fn make_polygons(&self, thickness: Distance) -> Polygon {
        self.to_polyline().make_polygons(thickness)
    }

    /// Length of the line segment
    pub fn length(&self) -> Distance {
        self.pt1().dist_to(self.pt2())
    }

    /// If two line segments intersect -- including endpoints -- return the point where they hit.
    /// Undefined if the two lines have more than one intersection point!
    // TODO Also return the distance along self
    pub fn intersection(&self, other: &Line) -> Option<Pt2D> {
        // From http://bryceboe.com/2006/10/23/line-segment-intersection-algorithm/
        if is_counter_clockwise(self.pt1(), other.pt1(), other.pt2())
            == is_counter_clockwise(self.pt2(), other.pt1(), other.pt2())
            || is_counter_clockwise(self.pt1(), self.pt2(), other.pt1())
                == is_counter_clockwise(self.pt1(), self.pt2(), other.pt2())
        {
            return None;
        }

        let hit = self.infinite().intersection(&other.infinite())?;
        if self.contains_pt(hit) {
            // TODO and other contains pt, then we dont need ccw check thing
            Some(hit)
        } else {
            // TODO Should be impossible, but I was hitting it somewhere
            println!(
                "{} and {} intersect, but first line doesn't contain_pt({})",
                self, other, hit
            );
            None
        }
    }

    /// Determine if two line segments intersect, but more so than just two endpoints touching.
    pub fn crosses(&self, other: &Line) -> bool {
        #[allow(clippy::suspicious_operation_groupings)] // false positive
        if self.pt1() == other.pt1()
            || self.pt1() == other.pt2()
            || self.pt2() == other.pt1()
            || self.pt2() == other.pt2()
        {
            return false;
        }
        self.intersection(other).is_some()
    }

    /// If the line segment intersects with an infinite line -- including endpoints -- return the
    /// point where they hit. Undefined if the segment and infinite line intersect at more than one
    /// point!
    // TODO Also return the distance along self
    pub fn intersection_infinite(&self, other: &InfiniteLine) -> Option<Pt2D> {
        let hit = self.infinite().intersection(other)?;
        if self.contains_pt(hit) {
            Some(hit)
        } else {
            None
        }
    }

    /// Perpendicularly shifts the line over to the right. Width must be non-negative.
    pub fn shift_right(&self, width: Distance) -> Line {
        assert!(width >= Distance::ZERO);
        let angle = self.angle().rotate_degs(90.0);
        Line::must_new(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    /// Perpendicularly shifts the line over to the left. Width must be non-negative.
    pub fn shift_left(&self, width: Distance) -> Line {
        assert!(width >= Distance::ZERO);
        let angle = self.angle().rotate_degs(-90.0);
        Line::must_new(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    /// Perpendicularly shifts the line to the right if positive or left if negative.
    pub fn shift_either_direction(&self, width: Distance) -> Line {
        if width >= Distance::ZERO {
            self.shift_right(width)
        } else {
            self.shift_left(-width)
        }
    }

    /// Returns a reversed line segment
    pub fn reversed(&self) -> Line {
        Line::must_new(self.pt2(), self.pt1())
    }

    /// The angle of the line segment, from the first to the second point
    pub fn angle(&self) -> Angle {
        self.pt1().angle_to(self.pt2())
    }

    /// Returns a point along the line segment, unless the distance exceeds the segment's length.
    pub fn dist_along(&self, dist: Distance) -> Result<Pt2D> {
        let len = self.length();
        if dist < Distance::ZERO || dist > len {
            bail!("dist_along({}) of a length {} line", dist, len);
        }
        self.percent_along(dist / len)
    }
    /// Equivalent to `self.dist_along(dist).unwrap()`. Use this to document an assertion at the
    /// call-site.
    pub fn must_dist_along(&self, dist: Distance) -> Pt2D {
        self.dist_along(dist).unwrap()
    }

    pub fn unbounded_dist_along(&self, dist: Distance) -> Pt2D {
        self.unbounded_percent_along(dist / self.length())
    }

    pub fn unbounded_percent_along(&self, percent: f64) -> Pt2D {
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
    }
    pub fn percent_along(&self, percent: f64) -> Result<Pt2D> {
        if !(0.0..=1.0).contains(&percent) {
            bail!("percent_along({}) of some line outside [0, 1]", percent);
        }
        Ok(self.unbounded_percent_along(percent))
    }

    pub fn slice(&self, from: Distance, to: Distance) -> Result<Line> {
        if from < Distance::ZERO || to < Distance::ZERO || from >= to {
            bail!("slice({}, {}) makes no sense", from, to);
        }
        Line::new(self.dist_along(from)?, self.dist_along(to)?)
    }

    /// Returns a subset of this line, with two percentages along the line's length.
    pub fn percent_slice(&self, from: f64, to: f64) -> Result<Line> {
        self.slice(from * self.length(), to * self.length())
    }

    pub fn middle(&self) -> Result<Pt2D> {
        self.dist_along(self.length() / 2.0)
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.dist_along_of_point(pt).is_some()
    }

    pub fn dist_along_of_point(&self, pt: Pt2D) -> Option<Distance> {
        let dist1 = self.pt1().raw_dist_to(pt);
        let dist2 = pt.raw_dist_to(self.pt2());
        let length = self.pt1().raw_dist_to(self.pt2());
        if (dist1 + dist2 - length).abs() < EPSILON_DIST.inner_meters() {
            Some(Distance::meters(dist1))
        } else {
            None
        }
    }
    pub fn percent_along_of_point(&self, pt: Pt2D) -> Option<f64> {
        let dist = self.dist_along_of_point(pt)?;
        Some(dist / self.length())
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Line::new(")?;
        writeln!(f, "  Pt2D::new({}, {}),", self.0.x(), self.0.y())?;
        writeln!(f, "  Pt2D::new({}, {}),", self.1.x(), self.1.y())?;
        write!(f, ")")
    }
}

fn is_counter_clockwise(pt1: Pt2D, pt2: Pt2D, pt3: Pt2D) -> bool {
    (pt3.y() - pt1.y()) * (pt2.x() - pt1.x()) > (pt2.y() - pt1.y()) * (pt3.x() - pt1.x())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct InfiniteLine(Pt2D, Pt2D);

impl InfiniteLine {
    /// Fails for parallel lines.
    // https://stackoverflow.com/a/565282 by way of
    // https://github.com/ucarion/line_intersection/blob/master/src/lib.rs
    pub fn intersection(&self, other: &InfiniteLine) -> Option<Pt2D> {
        #![allow(clippy::many_single_char_names)]
        fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
            a.0 * b.1 - a.1 * b.0
        }

        let p = self.0;
        let q = other.0;
        let r = (self.1.x() - self.0.x(), self.1.y() - self.0.y());
        let s = (other.1.x() - other.0.x(), other.1.y() - other.0.y());

        let r_cross_s = cross(r, s);
        let q_minus_p = (q.x() - p.x(), q.y() - p.y());
        //let q_minus_p_cross_r = cross(q_minus_p, r);

        if r_cross_s == 0.0 {
            // Parallel
            None
        } else {
            let t = cross(q_minus_p, (s.0 / r_cross_s, s.1 / r_cross_s));
            //let u = cross(q_minus_p, Pt2D::new(r.x() / r_cross_s, r.y() / r_cross_s));
            Some(Pt2D::new(p.x() + t * r.0, p.y() + t * r.1))
        }
    }

    pub fn from_pt_angle(pt: Pt2D, angle: Angle) -> InfiniteLine {
        Line::must_new(pt, pt.project_away(Distance::meters(1.0), angle)).infinite()
    }
}

impl fmt::Display for InfiniteLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "InfiniteLine::new(")?;
        writeln!(f, "  Pt2D::new({}, {}),", self.0.x(), self.0.y())?;
        writeln!(f, "  Pt2D::new({}, {}),", self.1.x(), self.1.y())?;
        write!(f, ")")
    }
}
