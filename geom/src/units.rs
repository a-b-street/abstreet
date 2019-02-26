use crate::{trim_f64, EPSILON_DIST};
use serde_derive::{Deserialize, Serialize};
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
    // TODO Different than EPSILON_DIST... the true minimum representable difference.
    pub const EPSILON: Distance = Distance::const_meters(0.0001);

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

    pub fn epsilon_eq(self, other: Distance) -> bool {
        (self - other).abs() <= EPSILON_DIST
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
        Duration::seconds(self.0 / other.0)
    }
}

// In seconds. Can be negative.
// TODO Naming is awkward. Can represent a moment in time or a duration.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Duration(f64);

// By construction, Duration is a finite f64 with trimmed precision.
impl Eq for Duration {}
impl Ord for Duration {
    fn cmp(&self, other: &Duration) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Duration {
    pub const ZERO: Duration = Duration::const_seconds(0.0);
    pub const EPSILON: Duration = Duration::const_seconds(0.0001);

    pub fn seconds(value: f64) -> Duration {
        if !value.is_finite() {
            panic!("Bad Duration {}", value);
        }

        Duration(trim_f64(value))
    }

    pub const fn const_seconds(value: f64) -> Duration {
        Duration(value)
    }

    pub fn min(self, other: Duration) -> Duration {
        if self <= other {
            self
        } else {
            other
        }
    }

    // TODO Remove if possible.
    pub fn inner_seconds(self) -> f64 {
        self.0
    }

    pub fn is_multiple_of(self, other: Duration) -> bool {
        self.inner_seconds() % other.inner_seconds() == 0.0
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

impl ops::Add for Duration {
    type Output = Duration;

    fn add(self, other: Duration) -> Duration {
        Duration::seconds(self.0 + other.0)
    }
}

impl ops::AddAssign for Duration {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl ops::SubAssign for Duration {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
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

// TODO Both of these work. Use a macro or crate to define both, so we don't have to worry about
// order for commutative things like multiplication. :P
impl ops::Mul<Duration> for f64 {
    type Output = Duration;

    fn mul(self, other: Duration) -> Duration {
        Duration::seconds(self * other.0)
    }
}

impl ops::Mul<Speed> for Duration {
    type Output = Distance;

    fn mul(self, other: Speed) -> Distance {
        Distance::meters(self.0 * other.0)
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
        Distance::meters(self.0 * other.0)
    }
}

impl ops::Div<Duration> for Speed {
    type Output = Acceleration;

    fn div(self, other: Duration) -> Acceleration {
        if other == Duration::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        Acceleration::meters_per_second_squared(self.0 / other.0)
    }
}

impl ops::Div<Acceleration> for Speed {
    type Output = Duration;

    fn div(self, other: Acceleration) -> Duration {
        if other == Acceleration::ZERO {
            panic!("Can't divide {} / {}", self, other);
        }
        Duration::seconds(self.0 / other.0)
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
        Speed::meters_per_second(self.0 * other.0)
    }
}

impl ops::Mul<Acceleration> for f64 {
    type Output = Acceleration;

    fn mul(self, other: Acceleration) -> Acceleration {
        Acceleration::meters_per_second_squared(self * other.0)
    }
}
