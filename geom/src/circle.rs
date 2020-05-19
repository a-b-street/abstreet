use crate::{Angle, Bounds, Distance, Polygon, Pt2D, Ring};
use serde::{Deserialize, Serialize};
use std::fmt;

const TRIANGLES_PER_CIRCLE: usize = 60;

#[derive(Serialize, Deserialize, Debug, Clone)]
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

    pub fn to_polygon(&self) -> Polygon {
        self.to_partial_polygon(1.0)
    }

    pub fn to_partial_polygon(&self, percent_full: f64) -> Polygon {
        let mut pts = vec![self.center];
        let mut indices = Vec::new();
        for i in 0..TRIANGLES_PER_CIRCLE {
            pts.push(self.center.project_away(
                self.radius,
                Angle::new_degs((i as f64) / (TRIANGLES_PER_CIRCLE as f64) * percent_full * 360.0),
            ));
            indices.push(0);
            indices.push(i + 1);
            if i != TRIANGLES_PER_CIRCLE - 1 {
                indices.push(i + 2);
            } else if percent_full == 1.0 {
                indices.push(1);
            } else {
                indices.pop();
                indices.pop();
            }
        }
        Polygon::precomputed(pts, indices)
    }

    pub fn outline(center: Pt2D, radius: Distance, thickness: Distance) -> Polygon {
        assert!(radius > thickness);

        // TODO This impl doesn't work because there's a weird edge
        if false {
            let mut pts = Circle::new(center, radius).to_polygon().points().clone();
            pts.push(pts[0]);
            return Ring::new(pts).make_polygons(thickness);
        }

        // TODO Argh this one also leaves a little piece missing, but it looks less bad. Fine.
        let bigger = Circle::new(center, radius).to_polygon();
        let smaller = Circle::new(center, radius - thickness).to_polygon();
        let mut polygons = bigger.difference(&smaller);
        let mut result = polygons.pop().unwrap();
        for p in polygons {
            result = result.union(p);
        }
        result
    }
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Circle({}, {})", self.center, self.radius)
    }
}
