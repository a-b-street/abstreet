use std::{cmp, ops};

use serde::{Deserialize, Serialize};

use crate::{deserialize_f64, serialize_f64, trim_f64, Distance, Duration, UnitFmt};

/// In meters per second. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Speed(
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")] f64,
);

// By construction, Speed is a finite f64 with trimmed precision.
impl Eq for Speed {}

#[allow(clippy::derive_ord_xor_partial_ord)] // false positive
impl Ord for Speed {
    fn cmp(&self, other: &Speed) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Speed {
    pub const ZERO: Speed = Speed::const_meters_per_second(0.0);

    pub fn meters_per_second(value: f64) -> Speed {
        if !value.is_finite() {
            panic!("Bad Speed {}", value);
        }

        Speed(trim_f64(value))
    }

    pub const fn const_meters_per_second(value: f64) -> Speed {
        Speed(value)
    }

    pub fn miles_per_hour(value: f64) -> Speed {
        Speed::meters_per_second(0.44704 * value)
    }

    pub fn km_per_hour(value: f64) -> Speed {
        Speed::meters_per_second(0.277778 * value)
    }

    pub fn from_dist_time(d: Distance, t: Duration) -> Speed {
        Speed::meters_per_second(d.inner_meters() / t.inner_seconds())
    }

    // TODO Remove if possible.
    pub fn inner_meters_per_second(self) -> f64 {
        self.0
    }

    pub fn to_miles_per_hour(self) -> f64 {
        self.0 * 2.23694
    }

    /// Describes the speed according to formatting rules.
    pub fn to_string(self, fmt: &UnitFmt) -> String {
        if fmt.metric {
            format!("{} km/h", (self.0 * 3.6).round())
        } else {
            format!("{} mph", self.to_miles_per_hour().round())
        }
    }
}

impl ops::Add for Speed {
    type Output = Speed;

    fn add(self, other: Speed) -> Speed {
        Speed::meters_per_second(self.0 + other.0)
    }
}

impl ops::Sub for Speed {
    type Output = Speed;

    fn sub(self, other: Speed) -> Speed {
        Speed::meters_per_second(self.0 - other.0)
    }
}

impl ops::Div for Speed {
    type Output = f64;

    fn div(self, other: Speed) -> f64 {
        self.0 / other.0
    }
}

impl ops::Neg for Speed {
    type Output = Speed;

    fn neg(self) -> Speed {
        Speed::meters_per_second(-self.0)
    }
}

impl ops::Mul<f64> for Speed {
    type Output = Speed;

    fn mul(self, scalar: f64) -> Speed {
        Speed::meters_per_second(self.0 * scalar)
    }
}

impl ops::Mul<Speed> for f64 {
    type Output = Speed;

    fn mul(self, other: Speed) -> Speed {
        Speed::meters_per_second(self * other.0)
    }
}

impl ops::Mul<Duration> for Speed {
    type Output = Distance;

    fn mul(self, other: Duration) -> Distance {
        Distance::meters(self.0 * other.inner_seconds())
    }
}
