use crate::{Angle, PolyLine, Polygon, Pt2D, EPSILON_DIST};
use dimensioned::si;
use serde_derive::{Deserialize, Serialize};
use std::fmt;

// Segment, technically. Should rename.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Line(Pt2D, Pt2D);

impl Line {
    // TODO only one place outside this crate calls this, try to fix maybe?
    pub fn new(pt1: Pt2D, pt2: Pt2D) -> Line {
        Line(pt1, pt2)
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

    pub fn make_polygons(&self, thickness: f64) -> Polygon {
        self.to_polyline().make_polygons(thickness)
    }

    // TODO One polygon, please :)
    pub fn make_arrow(&self, thickness: f64) -> Vec<Polygon> {
        let head_size = 2.0 * thickness;
        let angle = self.angle();
        let triangle_height = (head_size / 2.0).sqrt() * si::M;
        vec![
            Polygon::new(&vec![
                //self.pt2(),
                //self.pt2().project_away(head_size, angle.rotate_degs(-135.0)),
                self.reverse()
                    .dist_along(triangle_height)
                    .project_away(thickness / 2.0, angle.rotate_degs(90.0)),
                self.pt1()
                    .project_away(thickness / 2.0, angle.rotate_degs(90.0)),
                self.pt1()
                    .project_away(thickness / 2.0, angle.rotate_degs(-90.0)),
                self.reverse()
                    .dist_along(triangle_height)
                    .project_away(thickness / 2.0, angle.rotate_degs(-90.0)),
                //self.pt2().project_away(head_size, angle.rotate_degs(135.0)),
            ]),
            Polygon::new(&vec![
                self.pt2(),
                self.pt2()
                    .project_away(head_size, angle.rotate_degs(-135.0)),
                self.pt2().project_away(head_size, angle.rotate_degs(135.0)),
            ]),
        ]
    }

    // TODO valid to do euclidean distance on world-space points that're formed from
    // Haversine?
    pub fn length(&self) -> si::Meter<f64> {
        ((self.pt1().x() - self.pt2().x()).powi(2) + (self.pt1().y() - self.pt2().y()).powi(2))
            .sqrt()
            * si::M
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
            Some(hit)
        } else {
            // TODO This shouldn't be possible! :D
            println!(
                "{} and {} intersect, but first line doesn't contain_pt({})",
                self, other, hit
            );
            None
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

    pub fn shift_right(&self, width: f64) -> Line {
        assert!(width >= 0.0);
        let angle = self.angle().rotate_degs(90.0);
        Line(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    pub fn shift_left(&self, width: f64) -> Line {
        assert!(width >= 0.0);
        let angle = self.angle().rotate_degs(-90.0);
        Line(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    pub(crate) fn shift_either_direction(&self, width: f64) -> Line {
        if width >= 0.0 {
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

    pub fn dist_along(&self, dist: si::Meter<f64>) -> Pt2D {
        let len = self.length();
        if dist > len + EPSILON_DIST {
            panic!("cant do {} along a line of length {}", dist, len);
        }
        if len < EPSILON_DIST {
            // dist is also tiny because of the check above.
            return self.pt1();
        }

        let percent = (dist / len).value_unsafe;
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
        // TODO unit test
        /*
        let res_len = euclid_dist((pt1, &Pt2D::new(res[0], res[1])));
        if res_len != dist_along {
            println!("whats the delta btwn {} and {}?", res_len, dist_along);
        }
        */
    }

    pub fn unbounded_dist_along(&self, dist: si::Meter<f64>) -> Pt2D {
        let len = self.length();
        let percent = (dist / len).value_unsafe;
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
        // TODO unit test
        /*
        let res_len = euclid_dist((pt1, &Pt2D::new(res[0], res[1])));
        if res_len != dist_along {
            println!("whats the delta btwn {} and {}?", res_len, dist_along);
        }
        */
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        let dist = self.0.dist_to(pt) + pt.dist_to(self.1) - self.length();
        if dist < 0.0 * si::M {
            -dist < EPSILON_DIST
        } else {
            dist < EPSILON_DIST
        }
    }

    fn is_horizontal(&self) -> bool {
        (self.0.y() - self.1.y()).abs() < EPSILON_DIST.value_unsafe
    }

    fn is_vertical(&self) -> bool {
        (self.0.x() - self.1.x()).abs() < EPSILON_DIST.value_unsafe
    }

    pub fn dist_along_of_point(&self, pt: Pt2D) -> Option<si::Meter<f64>> {
        const PERCENT_EPSILON: f64 = 0.000_000_000_1;

        if !self.contains_pt(pt) {
            return None;
        }

        let percent1 = (pt.x() - self.pt1().x()) / (self.pt2().x() - self.pt1().x());
        let percent2 = (pt.y() - self.pt1().y()) / (self.pt2().y() - self.pt1().y());

        // TODO Urgh, special cases. Probably projecting a point onto the line could help?
        if self.is_horizontal() {
            Some(self.length() * percent1)
        } else if self.is_vertical() {
            Some(self.length() * percent2)
        } else if (percent1 - percent2).abs() < PERCENT_EPSILON {
            Some(self.length() * percent1)
        } else if percent1.is_nan() {
            Some(self.length() * percent2)
        } else if percent2.is_nan() {
            Some(self.length() * percent1)
        } else {
            None
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
    // https://en.wikipedia.org/wiki/Line%E2%80%93line_intersection#Given_two_points_on_each_line
    pub fn intersection(&self, other: &InfiniteLine) -> Option<Pt2D> {
        let x1 = self.0.x();
        let y1 = self.0.y();
        let x2 = self.1.x();
        let y2 = self.1.y();

        let x3 = other.0.x();
        let y3 = other.0.y();
        let x4 = other.1.x();
        let y4 = other.1.y();

        let numer_x = (x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4);
        let numer_y = (x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4);
        let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
        if denom == 0.0 {
            None
        } else {
            Some(Pt2D::new(numer_x / denom, numer_y / denom))
        }
    }
}
