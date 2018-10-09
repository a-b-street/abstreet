use dimensioned::si;
use std::fmt;
use {line_intersection, Angle, Pt2D, EPSILON_DIST};

// Segment, technically
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Line(Pt2D, Pt2D);

impl Line {
    // TODO only one place outside this crate calls this, try to fix maybe?
    pub fn new(pt1: Pt2D, pt2: Pt2D) -> Line {
        Line(pt1, pt2)
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

    // TODO valid to do euclidean distance on world-space points that're formed from
    // Haversine?
    pub fn length(&self) -> si::Meter<f64> {
        ((self.pt1().x() - self.pt2().x()).powi(2) + (self.pt1().y() - self.pt2().y()).powi(2))
            .sqrt()
            * si::M
    }

    pub fn intersection(&self, other: &Line) -> Option<Pt2D> {
        // TODO shoddy way of implementing this
        // TODO doesn't handle nearly parallel lines
        if !self.intersects(other) {
            None
        } else {
            line_intersection(self, other)
        }
    }

    pub fn shift(&self, width: f64) -> Line {
        let angle = self.pt1().angle_to(self.pt2()).rotate_degs(90.0);
        Line(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    pub fn reverse(&self) -> Line {
        Line(self.pt2(), self.pt1())
    }

    pub fn intersects(&self, other: &Line) -> bool {
        // From http://bryceboe.com/2006/10/23/line-segment-intersection-algorithm/
        is_counter_clockwise(self.pt1(), other.pt1(), other.pt2())
            != is_counter_clockwise(self.pt2(), other.pt1(), other.pt2())
            && is_counter_clockwise(self.pt1(), self.pt2(), other.pt1())
                != is_counter_clockwise(self.pt1(), self.pt2(), other.pt2())
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
        */    }

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
        */    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        let dist = Line(self.0, pt).length() + Line(pt, self.1).length() - self.length();
        if dist < 0.0 * si::M {
            -1.0 * dist < EPSILON_DIST
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
        const PERCENT_EPSILON: f64 = 0.0000000001;

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
        write!(f, "Line::new(\n")?;
        write!(f, "  Pt2D::new({}, {}),\n", self.0.x(), self.0.y())?;
        write!(f, "  Pt2D::new({}, {}),\n", self.1.x(), self.1.y())?;
        write!(f, ")")
    }
}

fn is_counter_clockwise(pt1: Pt2D, pt2: Pt2D, pt3: Pt2D) -> bool {
    (pt3.y() - pt1.y()) * (pt2.x() - pt1.x()) > (pt2.y() - pt1.y()) * (pt3.x() - pt1.x())
}

#[test]
fn test_dist_along_horiz_line() {
    let l = Line::new(
        Pt2D::new(147.17832753158294, 1651.034235433578),
        Pt2D::new(185.9754103560146, 1651.0342354335778),
    );
    let pt = Pt2D::new(179.1628455160347, 1651.0342354335778);

    assert!(l.contains_pt(pt));
    assert!(l.dist_along_of_point(pt).is_some());
}
