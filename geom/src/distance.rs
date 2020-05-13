use crate::{trim_f32, Duration, Speed};
use serde_derive::{Deserialize, Serialize};
use std::{cmp, f32, fmt, ops};

// In meters. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Distance(f32);

// By construction, Distance is a finite f32 with trimmed precision.
impl Eq for Distance {}
impl Ord for Distance {
    fn cmp(&self, other: &Distance) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Distance {
    pub const ZERO: Distance = Distance::const_meters(0.0);

    pub fn meters(value: f32) -> Distance {
        if !value.is_finite() {
            panic!("Bad Distance {}", value);
        }

        Distance(trim_f32(value))
    }

    // TODO Can't panic inside a const fn, seemingly. Don't pass in anything bad!
    pub const fn const_meters(value: f32) -> Distance {
        Distance(value)
    }

    pub fn inches(value: f32) -> Distance {
        Distance::meters(0.0254 * value)
    }

    pub fn miles(value: f32) -> Distance {
        Distance::meters(1609.34 * value)
    }

    pub fn centimeters(value: usize) -> Distance {
        Distance::meters((value as f32) / 100.0)
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
    pub fn inner_meters(self) -> f32 {
        self.0
    }

    pub fn describe_rounded(self) -> String {
        format!("{}m", self.0.round())
    }
}

impl fmt::Display for Distance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO commas every third place
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

impl ops::Mul<f32> for Distance {
    type Output = Distance;

    fn mul(self, scalar: f32) -> Distance {
        Distance::meters(self.0 * scalar)
    }
}

impl ops::Mul<Distance> for f32 {
    type Output = Distance;

    fn mul(self, other: Distance) -> Distance {
        Distance::meters(self * other.0)
    }
}

impl ops::Div<Distance> for Distance {
    type Output = f32;

    fn div(self, other: Distance) -> f32 {
        if other == Distance::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        self.0 / other.0
    }
}

impl ops::Div<f32> for Distance {
    type Output = Distance;

    fn div(self, scalar: f32) -> Distance {
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
