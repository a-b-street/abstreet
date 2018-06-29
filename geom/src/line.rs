use dimensioned::si;
use {util, Angle, Pt2D};

// Segment, technically
#[derive(Debug)]
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

    // TODO valid to do euclidean distance on world-space points that're formed from
    // Haversine?
    pub fn length(&self) -> si::Meter<f64> {
        ((self.pt1().x() - self.pt2().x()).powi(2) + (self.pt1().y() - self.pt2().y()).powi(2))
            .sqrt() * si::M
    }

    pub fn intersection(&self, other: &Line) -> Option<Pt2D> {
        // TODO shoddy way of implementing this
        // TODO doesn't handle nearly parallel lines
        if !self.intersects(other) {
            None
        } else {
            util::line_intersection(self, other)
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
        if dist > len + util::EPSILON_METERS {
            panic!("cant do {} along a line of length {}", dist, len);
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
}

fn is_counter_clockwise(pt1: Pt2D, pt2: Pt2D, pt3: Pt2D) -> bool {
    (pt3.y() - pt1.y()) * (pt2.x() - pt1.x()) > (pt2.y() - pt1.y()) * (pt3.x() - pt1.x())
}
