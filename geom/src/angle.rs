use std::fmt;

use serde::{Deserialize, Serialize};

/// An angle, stored in radians.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Angle(f64);

impl Angle {
    pub const ZERO: Angle = Angle(0.0);

    /// Create an angle in radians.
    // TODO Normalize here, and be careful about % vs euclid_rem
    pub fn new_rads(rads: f64) -> Angle {
        // Retain more precision for angles...
        Angle((rads * 10_000_000.0).round() / 10_000_000.0)
    }

    /// Create an angle in degrees.
    pub fn degrees(degs: f64) -> Angle {
        Angle::new_rads(degs.to_radians())
    }

    /// Invert the direction of this angle.
    pub fn opposite(self) -> Angle {
        Angle::new_rads(self.0 + std::f64::consts::PI)
    }

    pub(crate) fn invert_y(self) -> Angle {
        Angle::new_rads(2.0 * std::f64::consts::PI - self.0)
    }

    /// Rotates this angle by some degrees.
    pub fn rotate_degs(self, degrees: f64) -> Angle {
        Angle::new_rads(self.0 + degrees.to_radians())
    }

    /// Returns [0, 2pi)
    pub fn normalized_radians(self) -> f64 {
        if self.0 < 0.0 {
            // TODO Be more careful about how we store the angle, to make sure this works
            self.0 + (2.0 * std::f64::consts::PI)
        } else {
            self.0
        }
    }

    /// Returns [0, 360)
    pub fn normalized_degrees(self) -> f64 {
        self.normalized_radians().to_degrees()
    }

    /// Returns [-180, 180]
    pub fn simple_shortest_rotation_towards(self, other: Angle) -> f64 {
        // https://math.stackexchange.com/questions/110080/shortest-way-to-achieve-target-angle
        ((self.normalized_degrees() - other.normalized_degrees() + 540.0) % 360.0) - 180.0
    }

    /// Logically this returns [-180, 180], but keep in mind when we print this angle, it'll
    /// normalize to be [0, 360].
    pub fn shortest_rotation_towards(self, other: Angle) -> Angle {
        Angle::degrees(self.simple_shortest_rotation_towards(other))
    }

    /// True if this angle is within some degrees of another, accounting for rotation
    pub fn approx_eq(self, other: Angle, within_degrees: f64) -> bool {
        self.simple_shortest_rotation_towards(other).abs() < within_degrees
    }

    /// I don't know how to describe what this does. Use for rotating labels in map-space and making
    /// sure the text is never upside-down.
    pub fn reorient(self) -> Angle {
        let theta = self.normalized_degrees().rem_euclid(360.0);
        let mut result = self;
        if theta > 90.0 {
            result = result.opposite();
        }
        if theta > 270.0 {
            result = result.opposite();
        }
        result
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
        Angle::new_rads(self.0 + other.0)
    }
}

impl std::ops::Neg for Angle {
    type Output = Angle;

    fn neg(self) -> Angle {
        Angle::new_rads(-self.0)
    }
}

impl std::ops::Div<f64> for Angle {
    type Output = Angle;

    fn div(self, scalar: f64) -> Angle {
        if scalar == 0.0 {
            panic!("Can't divide {} / {}", self, scalar);
        }
        Angle::new_rads(self.0 / scalar)
    }
}

impl std::iter::Sum for Angle {
    fn sum<I>(iter: I) -> Angle
    where
        I: Iterator<Item = Angle>,
    {
        let mut sum = Angle::ZERO;
        for x in iter {
            sum = sum + x;
        }
        sum
    }
}
