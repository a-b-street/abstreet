use std::f64;
use std::fmt;

// Stores in radians
#[derive(Clone, Copy, Debug)]
pub struct Angle(f64);

impl Angle {
    pub(crate) fn new(rads: f64) -> Angle {
        Angle(rads)
    }

    pub fn opposite(&self) -> Angle {
        Angle(self.0 + f64::consts::PI)
    }

    pub fn rotate_degs(&self, degrees: f64) -> Angle {
        Angle(self.0 + degrees.to_radians())
    }

    pub fn normalized_radians(&self) -> f64 {
        if self.0 < 0.0 {
            self.0 + (2.0 * f64::consts::PI)
        } else {
            self.0
        }
    }

    pub fn normalized_degrees(&self) -> f64 {
        self.normalized_radians().to_degrees()
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Angle({} degrees)", self.normalized_degrees())
    }
}
