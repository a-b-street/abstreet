use std::{cmp, f64, fmt, ops};

use serde::{Deserialize, Serialize};

use crate::{deserialize_f64, serialize_f64, trim_f64, Duration, Speed, UnitFmt, EPSILON_DIST};

/// A distance, in meters. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Distance(
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")] f64,
);

// By construction, Distance is a finite f64 with trimmed precision.
impl Eq for Distance {}

#[allow(clippy::derive_ord_xor_partial_ord)] // false positive
impl Ord for Distance {
    fn cmp(&self, other: &Distance) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Distance {
    pub const ZERO: Distance = Distance::const_meters(0.0);

    /// Creates a distance in meters.
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

    /// Creates a distance in inches.
    pub fn inches(value: f64) -> Distance {
        Distance::meters(0.0254 * value)
    }

    /// Creates a distance in miles.
    pub fn miles(value: f64) -> Distance {
        Distance::meters(1609.34 * value)
    }

    /// Creates a distance in centimeters.
    pub fn centimeters(value: usize) -> Distance {
        Distance::meters((value as f64) / 100.0)
    }

    /// Creates a distance in feet.
    pub fn feet(value: f64) -> Distance {
        Distance::meters(value * 0.3048)
    }

    /// Returns the absolute value of this distance.
    pub fn abs(self) -> Distance {
        if self.0 > 0.0 {
            self
        } else {
            Distance(-self.0)
        }
    }

    /// Returns the square root of this distance.
    pub fn sqrt(self) -> Distance {
        Distance::meters(self.0.sqrt())
    }

    /// Returns the distance in meters. Prefer to work with type-safe `Distance`s.
    // TODO Remove if possible.
    pub fn inner_meters(self) -> f64 {
        self.0
    }

    /// Returns the distance in feet.
    pub fn to_feet(self) -> f64 {
        self.0 * 3.28084
    }

    /// Returns the distance in miles.
    pub fn to_miles(self) -> f64 {
        self.to_feet() / 5280.0
    }

    /// Describes the distance according to formatting rules. Rounds to 1 decimal place for both
    /// small (feet and meters) and large (miles and kilometers) units.
    pub fn to_string(self, fmt: &UnitFmt) -> String {
        if fmt.metric {
            if self.0 < 1000.0 {
                format!("{}m", (self.0 * 10.0).round() / 10.0)
            } else {
                let km = self.0 / 1000.0;
                format!("{}km", (km * 10.0).round() / 10.0)
            }
        } else {
            let feet = self.to_feet();
            let miles = self.to_miles();
            if miles >= 0.1 {
                format!("{} miles", (miles * 10.0).round() / 10.0)
            } else {
                format!("{} ft", (feet * 10.0).round() / 10.0)
            }
        }
    }

    /// Calculates a percentage, usually in [0.0, 1.0], of self / other. If the denominator is
    /// zero, returns 0%.
    pub fn safe_percent(self, other: Distance) -> f64 {
        if other == Distance::ZERO {
            return 0.0;
        }
        self / other
    }

    /// Rounds this distance up to a higher, more "even" value to use for buckets along a plot's
    /// axis. Always rounds for imperial units (feet).
    pub fn round_up_for_axis(self) -> Distance {
        let ft = self.to_feet();
        let miles = ft / 5280.0;
        if ft <= 0.0 {
            Distance::ZERO
        } else if ft <= 10.0 {
            Distance::feet(ft.ceil())
        } else if ft <= 100.0 {
            Distance::feet(10.0 * (ft / 10.0).ceil())
        } else if miles < 0.1 {
            Distance::feet(100.0 * (ft / 100.0).ceil())
        } else if miles <= 1.0 {
            Distance::miles((miles * 10.0).ceil() / 10.0)
        } else if miles <= 10.0 {
            Distance::miles(miles.ceil())
        } else if miles <= 100.0 {
            Distance::miles(10.0 * (miles / 10.0).ceil())
        } else {
            self
        }
    }

    pub(crate) fn to_u64(self) -> u64 {
        (self.0 / EPSILON_DIST.0) as u64
    }

    pub(crate) fn from_u64(x: u64) -> Distance {
        (x as f64) * EPSILON_DIST
    }
}

impl fmt::Display for Distance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

impl ops::MulAssign<f64> for Distance {
    fn mul_assign(&mut self, other: f64) {
        *self = *self * other;
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
            panic!("Can't divide {} / 0 mph", self);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_up_for_axis() {
        let fmt = UnitFmt {
            metric: false,
            round_durations: false,
        };

        for (input, expected) in [
            (-3.0, 0.0),
            (0.0, 0.0),
            (3.2, 4.0),
            (30.2, 40.0),
            (300.2, 400.0),
            (
                Distance::miles(0.13).to_feet(),
                Distance::miles(0.2).to_feet(),
            ),
            (
                Distance::miles(0.64).to_feet(),
                Distance::miles(0.7).to_feet(),
            ),
            (
                Distance::miles(2.6).to_feet(),
                Distance::miles(3.0).to_feet(),
            ),
            (
                Distance::miles(2.9).to_feet(),
                Distance::miles(3.0).to_feet(),
            ),
        ] {
            assert_eq!(
                Distance::feet(input).round_up_for_axis().to_string(&fmt),
                Distance::feet(expected).to_string(&fmt)
            );
        }
    }
}
