use crate::{Angle, Bounds, Distance, Polygon, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
pub struct Circle {
    pub center: Pt2D,
    pub radius: Distance,
}

impl Circle {
    pub fn new(center: Pt2D, radius: Distance) -> Circle {
        Circle { center, radius }
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        // avoid sqrt by squaring radius instead
        (pt.x() - self.center.x()).powi(2) + (pt.y() - self.center.y()).powi(2)
            < self.radius.inner_meters().powi(2)
    }

    pub fn get_bounds(&self) -> Bounds {
        Bounds {
            min_x: self.center.x() - self.radius.inner_meters(),
            max_x: self.center.x() + self.radius.inner_meters(),
            min_y: self.center.y() - self.radius.inner_meters(),
            max_y: self.center.y() + self.radius.inner_meters(),
        }
    }

    pub fn to_polygon(&self, num_triangles: usize) -> Polygon {
        let mut pts = vec![self.center];
        let mut indices = Vec::new();
        for i in 0..num_triangles {
            pts.push(self.center.project_away(
                self.radius,
                Angle::new_degs((i as f64) / (num_triangles as f64) * 360.0),
            ));
            indices.push(0);
            indices.push(i + 1);
            if i != num_triangles - 1 {
                indices.push(i + 2);
            } else {
                indices.push(1);
            }
        }
        Polygon::precomputed(pts, indices)
    }
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Circle({}, {})", self.center, self.radius)
    }
}
