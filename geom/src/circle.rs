use std::fmt;
use {Bounds, Pt2D};

#[derive(Serialize, Deserialize, Debug)]
pub struct Circle {
    pub center: Pt2D,
    pub radius: f64,
}

impl Circle {
    pub fn new(center: Pt2D, radius: f64) -> Circle {
        Circle { center, radius }
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        // avoid sqrt by squaring radius instead
        (pt.x() - self.center.x()).powi(2) + (pt.y() - self.center.y()).powi(2)
            < self.radius.powi(2)
    }

    pub fn get_bounds(&self) -> Bounds {
        Bounds {
            min_x: self.center.x() - self.radius,
            max_x: self.center.x() + self.radius,
            min_y: self.center.y() - self.radius,
            max_y: self.center.y() + self.radius,
            represents_world_space: false,
        }
    }
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Circle({}, {})", self.center, self.radius)
    }
}
