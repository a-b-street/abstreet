use crate::{Angle, Distance, PolyLine, Polygon, Pt2D, EPSILON_DIST};
use geo::prelude::ClosestPoint;
use serde::{Deserialize, Serialize};
use std::fmt;

// Segment, technically. Should rename.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Line(Pt2D, Pt2D);

impl Line {
    pub fn new(pt1: Pt2D, pt2: Pt2D) -> Line {
        let len = pt1.dist_to(pt2);
        if len < EPSILON_DIST {
            panic!("Tiny line with length {}", len);
        }
        Line(pt1, pt2)
    }

    pub fn maybe_new(pt1: Pt2D, pt2: Pt2D) -> Option<Line> {
        if pt1 == pt2 {
            return None;
        }
        Some(Line::new(pt1, pt2))
    }

    pub fn infinite(&self) -> InfiniteLine {
        InfiniteLine(self.0, self.1)
    }

    // TODO we call these frequently here; unnecessary copies?
    pub fn pt1(&self) -> Pt2D {
        self.0
    }

    pub fn pt2(&self) -> Pt2D {
        self.1
    }

    pub fn points(&self) -> Vec<Pt2D> {
        vec![self.0, self.1]
    }

    pub fn to_polyline(&self) -> PolyLine {
        PolyLine::new(self.points())
    }

    pub fn make_polygons(&self, thickness: Distance) -> Polygon {
        self.to_polyline().make_polygons(thickness)
    }

    pub fn length(&self) -> Distance {
        self.pt1().dist_to(self.pt2())
    }

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
            panic!(
                "{} and {} intersect, but first line doesn't contain_pt({})",
                self, other, hit
            );
        }
    }

    // TODO Also return the distance along self
    pub fn intersection_infinite(&self, other: &InfiniteLine) -> Option<Pt2D> {
        let hit = self.infinite().intersection(other)?;
        if self.contains_pt(hit) {
            Some(hit)
        } else {
            None
        }
    }

    pub fn shift_right(&self, width: Distance) -> Line {
        assert!(width >= Distance::ZERO);
        let angle = self.angle().rotate_degs(90.0);
        Line(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    pub fn shift_left(&self, width: Distance) -> Line {
        assert!(width >= Distance::ZERO);
        let angle = self.angle().rotate_degs(-90.0);
        Line(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    pub fn shift_either_direction(&self, width: Distance) -> Line {
        if width >= Distance::ZERO {
            self.shift_right(width)
        } else {
            self.shift_left(-width)
        }
    }

    pub fn reverse(&self) -> Line {
        Line(self.pt2(), self.pt1())
    }

    pub fn angle(&self) -> Angle {
        self.pt1().angle_to(self.pt2())
    }

    pub fn dist_along(&self, dist: Distance) -> Pt2D {
        let len = self.length();
        if dist > len + EPSILON_DIST {
            panic!("cant do {} along a line of length {}", dist, len);
        }
        if len < EPSILON_DIST {
            // dist is also tiny because of the check above.
            return self.pt1();
        }

        self.percent_along(dist / len)
    }

    pub fn unbounded_percent_along(&self, percent: f64) -> Pt2D {
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
    }
    pub fn percent_along(&self, percent: f64) -> Pt2D {
        assert!(percent >= 0.0 && percent <= 1.0);
        self.unbounded_percent_along(percent)
    }

    pub fn unbounded_dist_along(&self, dist: Distance) -> Pt2D {
        let len = self.length();
        let percent = dist / len;
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
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

    // Returns a point on the line segment.
    pub fn project_pt(&self, pt: Pt2D) -> Pt2D {
        let line: geo::LineString<f64> = vec![
            geo::Point::new(self.0.x(), self.0.y()),
            geo::Point::new(self.1.x(), self.1.y()),
        ]
        .into();
        match line.closest_point(&geo::Point::new(pt.x(), pt.y())) {
            geo::Closest::Intersection(hit) | geo::Closest::SinglePoint(hit) => {
                Pt2D::new(hit.x(), hit.y())
            }
            geo::Closest::Indeterminate => unreachable!(),
        }
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
    // Fails for parallel lines.
    // https://stackoverflow.com/a/565282 by way of
    // https://github.com/ucarion/line_intersection/blob/master/src/lib.rs
    pub fn intersection(&self, other: &InfiniteLine) -> Option<Pt2D> {
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
}

impl fmt::Display for InfiniteLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "InfiniteLine::new(")?;
        writeln!(f, "  Pt2D::new({}, {}),", self.0.x(), self.0.y())?;
        writeln!(f, "  Pt2D::new({}, {}),", self.1.x(), self.1.y())?;
        write!(f, ")")
    }
}
