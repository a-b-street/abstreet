use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{Angle, Bounds, Distance, Polygon, Pt2D, Ring, Tessellation};

const TRIANGLES_PER_CIRCLE: usize = 60;

/// A circle, defined by a center and radius.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Circle {
    pub center: Pt2D,
    pub radius: Distance,
}

impl Circle {
    /// Creates a circle.
    pub fn new(center: Pt2D, radius: Distance) -> Circle {
        Circle { center, radius }
    }

    /// True if the point is inside the circle.
    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        // avoid sqrt by squaring radius instead
        (pt.x() - self.center.x()).powi(2) + (pt.y() - self.center.y()).powi(2)
            < self.radius.inner_meters().powi(2)
    }

    /// Get the boundary containing this circle.
    pub fn get_bounds(&self) -> Bounds {
        Bounds {
            min_x: self.center.x() - self.radius.inner_meters(),
            max_x: self.center.x() + self.radius.inner_meters(),
            min_y: self.center.y() - self.radius.inner_meters(),
            max_y: self.center.y() + self.radius.inner_meters(),
        }
    }

    /// Renders the circle as a polygon.
    pub fn to_polygon(&self) -> Polygon {
        self.to_ring().into_polygon()
    }

    /// Renders some percent, between [0, 1], of the circle. The shape starts from 0 degrees.
    pub fn to_partial_tessellation(&self, percent_full: f64) -> Tessellation {
        #![allow(clippy::float_cmp)]
        assert!((0. ..=1.).contains(&percent_full));
        let mut pts = vec![self.center];
        let mut indices = Vec::new();
        for i in 0..TRIANGLES_PER_CIRCLE {
            pts.push(self.center.project_away(
                self.radius,
                Angle::degrees((i as f64) / (TRIANGLES_PER_CIRCLE as f64) * percent_full * 360.0),
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
        Tessellation::new(pts, indices)
    }

    /// Returns the ring around the circle.
    fn to_ring(&self) -> Ring {
        let mut pts: Vec<Pt2D> = (0..=TRIANGLES_PER_CIRCLE)
            .map(|i| {
                self.center.project_away(
                    self.radius,
                    Angle::degrees((i as f64) / (TRIANGLES_PER_CIRCLE as f64) * 360.0),
                )
            })
            .collect();
        // With some radii, we get duplicate adjacent points
        pts.dedup();
        Ring::must_new(pts)
    }

    /// Creates an outline around the circle, strictly contained with the circle's original radius.
    pub fn to_outline(&self, thickness: Distance) -> Result<Polygon> {
        if self.radius <= thickness {
            bail!(
                "Can't make Circle outline with radius {} and thickness {}",
                self.radius,
                thickness
            );
        }

        let bigger = self.to_ring();
        let smaller = Circle::new(self.center, self.radius - thickness).to_ring();
        Ok(Polygon::with_holes(bigger, vec![smaller]))
    }
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Circle({}, {})", self.center, self.radius)
    }
}
