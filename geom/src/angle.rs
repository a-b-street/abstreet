use serde_derive::{Deserialize, Serialize};
use std;
use std::f64;
use std::fmt;

// Stores in radians
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Angle(f64);

impl Angle {
    pub(crate) fn new(rads: f64) -> Angle {
        // Retain more precision for angles...
        Angle((rads * 10_000_000.0).round() / 10_000_000.0)
    }

    pub fn new_degs(degs: f64) -> Angle {
        Angle::new(degs.to_radians())
    }

    pub fn opposite(self) -> Angle {
        Angle::new(self.0 + f64::consts::PI)
    }

    pub fn rotate_degs(self, degrees: f64) -> Angle {
        Angle::new(self.0 + degrees.to_radians())
    }

    pub fn normalized_radians(self) -> f64 {
        if self.0 < 0.0 {
            self.0 + (2.0 * f64::consts::PI)
        } else {
            self.0
        }
    }

    pub fn normalized_degrees(self) -> f64 {
        self.normalized_radians().to_degrees()
    }

    // Logically this returns [-180, 180], but keep in mind when we print this angle, it'll
    // normalize to be [0, 360].
    pub fn shortest_rotation_towards(self, other: Angle) -> Angle {
        // https://math.stackexchange.com/questions/110080/shortest-way-to-achieve-target-angle
        Angle::new_degs(
            ((self.normalized_degrees() - other.normalized_degrees() + 540.0) % 360.0) - 180.0,
        )
    }

    pub fn approx_eq(self, other: Angle, within_degrees: f64) -> bool {
        // https://math.stackexchange.com/questions/110080/shortest-way-to-achieve-target-angle
        // This yields [-180, 180]
        let rotation =
            ((self.normalized_degrees() - other.normalized_degrees() + 540.0) % 360.0) - 180.0;
        rotation.abs() < within_degrees
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Angle({} degrees)", self.normalized_degrees())
    }
}

impl std::ops::Add for Angle {
    type Output = Angle;

    fn add(self, other: Angle) -> Angle {
        Angle::new(self.0 + other.0)
    }
}

impl std::ops::Sub for Angle {
    type Output = Angle;

    fn sub(self, other: Angle) -> Angle {
        Angle::new(self.0 - other.0)
    }
}
