use crate::{trim_f64, Duration, Speed};
use serde::{Deserialize, Serialize};
use std::{cmp, f64, fmt, ops};

// In meters. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Distance(f64);

// By construction, Distance is a finite f64 with trimmed precision.
impl Eq for Distance {}
impl Ord for Distance {
    fn cmp(&self, other: &Distance) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Distance {
    pub const ZERO: Distance = Distance::const_meters(0.0);

    pub fn meters(value: f64) -> Distance {
        if !value.is_finite() {
            panic!("Bad Distance {}", value);
        }

        Distance(trim_f64(value))
    }

    // TODO Can't panic inside a const fn, seemingly. Don't pass in anything bad!
    pub const fn const_meters(value: f64) -> Distance {
        Distance(value)
    }

    pub fn inches(value: f64) -> Distance {
        Distance::meters(0.0254 * value)
    }

    pub fn miles(value: f64) -> Distance {
        Distance::meters(1609.34 * value)
    }

    pub fn centimeters(value: usize) -> Distance {
        Distance::meters((value as f64) / 100.0)
    }

    pub fn abs(self) -> Distance {
        if self.0 > 0.0 {
            self
        } else {
            Distance(-self.0)
        }
    }

    pub fn sqrt(self) -> Distance {
        Distance::meters(self.0.sqrt())
    }

    // TODO Remove if possible.
    pub fn inner_meters(self) -> f64 {
        self.0
    }

    // TODO Store a bit in Maps to mark if they're in the US or not, plumb here to use meters
    pub fn describe_rounded(self) -> String {
        let feet = self.0 * 3.28084;
        let miles = feet / 5280.0;
        if miles >= 0.1 {
            format!("{} miles", (miles * 10.0).round() / 10.0)
        } else {
            format!("{} ft", feet.round())
        }
    }
}

impl fmt::Display for Distance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO This is harder to localize
        write!(f, "{}m", self.0)
    }
}

impl ops::Add for Distance {
    type Output = Distance;

    fn add(self, other: Distance) -> Distance {
        Distance::meters(self.0 + other.0)
    }
}

impl ops::AddAssign for Distance {
    fn add_assign(&mut self, other: Distance) {
        *self = *self + other;
    }
}

impl ops::Sub for Distance {
    type Output = Distance;

    fn sub(self, other: Distance) -> Distance {
        Distance::meters(self.0 - other.0)
    }
}

impl ops::Neg for Distance {
    type Output = Distance;

    fn neg(self) -> Distance {
        Distance::meters(-self.0)
    }
}

impl ops::SubAssign for Distance {
    fn sub_assign(&mut self, other: Distance) {
        *self = *self - other;
    }
}

impl ops::Mul<f64> for Distance {
    type Output = Distance;

    fn mul(self, scalar: f64) -> Distance {
        Distance::meters(self.0 * scalar)
    }
}

impl ops::Mul<Distance> for f64 {
    type Output = Distance;

    fn mul(self, other: Distance) -> Distance {
        Distance::meters(self * other.0)
    }
}

impl ops::Div<Distance> for Distance {
    type Output = f64;

    fn div(self, other: Distance) -> f64 {
        if other == Distance::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        self.0 / other.0
    }
}

impl ops::Div<f64> for Distance {
    type Output = Distance;

    fn div(self, scalar: f64) -> Distance {
        if scalar == 0.0 {
            panic!("Can't divide {} / {}", self, scalar);
        }
        Distance::meters(self.0 / scalar)
    }
}

impl ops::Div<Speed> for Distance {
    type Output = Duration;

    fn div(self, other: Speed) -> Duration {
        if other == Speed::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        Duration::seconds(self.0 / other.inner_meters_per_second())
    }
}

impl std::iter::Sum for Distance {
    fn sum<I>(iter: I) -> Distance
    where
        I: Iterator<Item = Distance>,
    {
        let mut sum = Distance::ZERO;
        for x in iter {
            sum += x;
        }
        sum
    }
}

impl Default for Distance {
    fn default() -> Distance {
        Distance::ZERO
    }
}
