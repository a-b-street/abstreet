use crate::{trim_f64, Distance, Speed};
use serde_derive::{Deserialize, Serialize};
use std::{cmp, f64, ops};

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

    pub fn minutes(mins: usize) -> Duration {
        Duration::seconds((mins as f64) * 60.0)
    }

    pub const fn const_seconds(value: f64) -> Duration {
        Duration(value)
    }

    pub fn to_u64(self) -> u64 {
        (self.0 / Duration::EPSILON.0) as u64
    }

    pub fn from_u64(x: u64) -> Duration {
        (x as f64) * Duration::EPSILON
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

    // TODO Why have these two forms? Consolidate
    pub fn parse(string: &str) -> Option<Duration> {
        let parts: Vec<&str> = string.split(':').collect();
        if parts.is_empty() {
            return None;
        }

        let mut seconds: f64 = 0.0;
        if parts.last().unwrap().contains('.') {
            let last_parts: Vec<&str> = parts.last().unwrap().split('.').collect();
            if last_parts.len() != 2 {
                return None;
            }
            seconds += last_parts[1].parse::<f64>().ok()? / 10.0;
            seconds += last_parts[0].parse::<f64>().ok()?;
        } else {
            seconds += parts.last().unwrap().parse::<f64>().ok()?;
        }

        match parts.len() {
            1 => Some(Duration::seconds(seconds)),
            2 => {
                seconds += 60.0 * parts[0].parse::<f64>().ok()?;
                Some(Duration(seconds))
            }
            3 => {
                seconds += 60.0 * parts[1].parse::<f64>().ok()?;
                seconds += 3600.0 * parts[0].parse::<f64>().ok()?;
                Some(Duration(seconds))
            }
            _ => None,
        }
    }

    /*pub fn parse_filename(string: &str) -> Option<Duration> {
        // TODO lazy_static! {
        let regex = Regex::new(r"(\d+)h(\d+)m(\d+)\.(\d+)s").unwrap();

        let caps = regex.captures(string)?;
        let hours = 3600.0 * caps[1].parse::<f64>().ok()?;
        let minutes = 60.0 * caps[2].parse::<f64>().ok()?;
        let seconds = caps[3].parse::<f64>().ok()?;
        let ms = caps[4].parse::<f64>().ok()? / 10.0;

        Some(Duration::seconds(hours + minutes + seconds + ms))
    }*/

    // (hours, minutes, seconds, centiseconds)
    fn get_parts(self) -> (usize, usize, usize, usize) {
        let mut remainder = self.inner_seconds();
        let hours = (remainder / 3600.0).floor();
        remainder -= hours * 3600.0;
        let minutes = (remainder / 60.0).floor();
        remainder -= minutes * 60.0;
        let seconds = remainder.floor();
        remainder -= seconds;
        let centis = (remainder / 0.1).round();

        (
            hours as usize,
            minutes as usize,
            seconds as usize,
            centis as usize,
        )
    }

    pub fn as_filename(self) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        format!(
            "{0:02}h{1:02}m{2:02}.{3:01}s",
            hours, minutes, seconds, remainder
        )
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        write!(
            f,
            "{0:02}:{1:02}:{2:02}.{3:01}",
            hours, minutes, seconds, remainder
        )
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

// TODO If the priority queue doesn't need this, get rid of it.
impl ops::Neg for Duration {
    type Output = Duration;

    fn neg(self) -> Duration {
        Duration::seconds(-self.0)
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
        Distance::meters(self.0 * other.inner_meters_per_second())
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
