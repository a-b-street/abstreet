use serde_derive::{Deserialize, Serialize};
use std::{fmt, ops};

// In meters. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Distance(f64);

impl Distance {
    pub const ZERO: Distance = Distance::const_meters(0.0);

    pub fn meters(value: f64) -> Distance {
        if !value.is_finite() {
            panic!("Bad Distance {}", value);
        }

        Distance(value)
    }

    // TODO Can't panic inside a const fn, seemingly. Don't pass in anything bad!
    pub const fn const_meters(value: f64) -> Distance {
        Distance(value)
    }

    pub fn abs(self) -> Distance {
        if self.0 > 0.0 {
            self
        } else {
            Distance(-self.0)
        }
    }

    // TODO Only used within this crate for NotNan construction and GPS->Pt2D.
    pub(crate) fn inner(self) -> f64 {
        self.0
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

impl ops::SubAssign for Distance {
    fn sub_assign(&mut self, other: Distance) {
        *self = *self - other;
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

    fn div(self, other: f64) -> Distance {
        if other == 0.0 {
            panic!("Can't divide {} / {}", self, other);
        }
        Distance::meters(self.0 / other)
    }
}

// In seconds. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Duration(f64);

impl Duration {
    pub fn seconds(value: f64) -> Duration {
        if !value.is_finite() {
            panic!("Bad Duration {}", value);
        }

        Duration(value)
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

// In meters per second. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Speed(f64);

impl Speed {
    pub fn meters_per_second(value: f64) -> Speed {
        if !value.is_finite() {
            panic!("Bad Speed {}", value);
        }

        Speed(value)
    }
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}m/s", self.0)
    }
}

// In meters per second^2. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Acceleration(f64);

impl Acceleration {
    pub fn meters_per_second_squared(value: f64) -> Acceleration {
        if !value.is_finite() {
            panic!("Bad Acceleration {}", value);
        }

        Acceleration(value)
    }
}

impl fmt::Display for Acceleration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}m/s^2", self.0)
    }
}
