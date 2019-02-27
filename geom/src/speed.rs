use crate::{trim_f64, Acceleration, Distance, Duration, EPSILON_DIST};
use serde_derive::{Deserialize, Serialize};
use std::{f64, fmt, ops};

// In meters per second. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Speed(f64);

impl Speed {
    pub const ZERO: Speed = Speed::const_meters_per_second(0.0);

    // Is a speed effectively zero based on the timestep?
    // TODO Probably better to tweak the rounding so that uselessly tiny speeds round to 0.
    pub fn is_zero(self, timestep: Duration) -> bool {
        self * timestep <= EPSILON_DIST
    }

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

    // TODO Remove if possible.
    pub fn inner_meters_per_second(self) -> f64 {
        self.0
    }

    pub fn max(self, other: Speed) -> Speed {
        if self >= other {
            self
        } else {
            other
        }
    }

    pub fn min(self, other: Speed) -> Speed {
        if self <= other {
            self
        } else {
            other
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

impl ops::Div<Duration> for Speed {
    type Output = Acceleration;

    fn div(self, other: Duration) -> Acceleration {
        if other == Duration::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        Acceleration::meters_per_second_squared(self.0 / other.inner_seconds())
    }
}

impl ops::Div<Acceleration> for Speed {
    type Output = Duration;

    fn div(self, other: Acceleration) -> Duration {
        if other == Acceleration::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        Duration::seconds(self.0 / other.inner_meters_per_second_squared())
    }
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}m/s", self.0)
    }
}
