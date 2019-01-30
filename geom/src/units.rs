use serde_derive::{Deserialize, Serialize};
use std::fmt;

// In meters. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Distance(f64);

impl Distance {
    pub fn meters(value: f64) -> Distance {
        Distance(value)
    }
}

impl fmt::Display for Distance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO commas every third place
        write!(f, "{}m", self.0)
    }
}

// In seconds. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Duration(f64);

impl Duration {
    pub fn seconds(value: f64) -> Duration {
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
        Acceleration(value)
    }
}

impl fmt::Display for Acceleration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}m/s^2", self.0)
    }
}
