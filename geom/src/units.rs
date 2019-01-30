use ordered_float::NotNan;
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

    // TODO Remove by making Distance itself Ord.
    pub fn as_ordered(self) -> NotNan<f64> {
        NotNan::new(self.0).unwrap()
    }

    // TODO Remove if possible.
    pub fn inner_meters(self) -> f64 {
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

impl ops::Mul<f64> for Distance {
    type Output = Distance;

    fn mul(self, scalar: f64) -> Distance {
        Distance::meters(self.0 * scalar)
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

    pub const fn const_seconds(value: f64) -> Duration {
        Duration(value)
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

impl ops::Sub for Duration {
    type Output = Duration;

    fn sub(self, other: Duration) -> Duration {
        Duration::seconds(self.0 - other.0)
    }
}

impl ops::Mul<f64> for Duration {
    type Output = Duration;

    fn mul(self, other: f64) -> Duration {
        Duration::seconds(self.0 * other)
    }
}

impl ops::Div<Duration> for Duration {
    type Output = f64;

    fn div(self, other: Duration) -> f64 {
        if other.0 == 0.0 {
            panic!("Can't divide {} / {}", self, other);
        }
        self.0 / other.0
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

    pub fn miles_per_hour(value: f64) -> Speed {
        Speed::meters_per_second(0.44704 * value)
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
