use crate::{trim_f64, Duration, Speed};
use serde_derive::{Deserialize, Serialize};
use std::{cmp, f64, fmt, ops};

// In meters per second^2. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Acceleration(f64);

// By construction, Acceleration is a finite f64 with trimmed precision.
impl Eq for Acceleration {}
impl Ord for Acceleration {
    fn cmp(&self, other: &Acceleration) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Acceleration {
    pub const ZERO: Acceleration = Acceleration::const_meters_per_second_squared(0.0);

    pub fn meters_per_second_squared(value: f64) -> Acceleration {
        if !value.is_finite() {
            panic!("Bad Acceleration {}", value);
        }

        Acceleration(trim_f64(value))
    }

    pub const fn const_meters_per_second_squared(value: f64) -> Acceleration {
        Acceleration(value)
    }

    pub fn min(self, other: Acceleration) -> Acceleration {
        if self <= other {
            self
        } else {
            other
        }
    }

    // TODO Remove if possible.
    pub fn inner_meters_per_second_squared(self) -> f64 {
        self.0
    }
}

impl fmt::Display for Acceleration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}m/s^2", self.0)
    }
}

impl ops::Mul<Duration> for Acceleration {
    type Output = Speed;

    fn mul(self, other: Duration) -> Speed {
        Speed::meters_per_second(self.0 * other.inner_seconds())
    }
}

impl ops::Mul<Acceleration> for f64 {
    type Output = Acceleration;

    fn mul(self, other: Acceleration) -> Acceleration {
        Acceleration::meters_per_second_squared(self * other.0)
    }
}
