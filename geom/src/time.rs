use std::{cmp, ops};

use anyhow::Result;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{deserialize_f64, serialize_f64, trim_f64, Duration};

/// In seconds since midnight. Can't be negative.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Time(
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")] f64,
);

// By construction, Time is a finite f64 with trimmed precision.
impl Eq for Time {}

#[allow(clippy::derive_ord_xor_partial_ord)] // false positive
impl Ord for Time {
    fn cmp(&self, other: &Time) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[allow(clippy::derive_hash_xor_eq)] // false positive
impl std::hash::Hash for Time {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        NotNan::new(self.0).unwrap().hash(state);
    }
}

impl Time {
    pub const START_OF_DAY: Time = Time(0.0);

    // No direct public constructors. Explicitly do Time::START_OF_DAY + duration.
    fn seconds_since_midnight(value: f64) -> Time {
        if !value.is_finite() || value < 0.0 {
            panic!("Bad Time {}", value);
        }

        Time(trim_f64(value))
    }

    /// (hours, minutes, seconds, centiseconds)
    fn get_parts(self) -> (usize, usize, usize, usize) {
        let mut remainder = self.0;
        let hours = (remainder / 3600.0).floor();
        remainder -= hours * 3600.0;
        let minutes = (remainder / 60.0).floor();
        remainder -= minutes * 60.0;
        let seconds = remainder.floor();
        remainder -= seconds;
        let centis = (remainder / 0.1).floor();

        (
            hours as usize,
            minutes as usize,
            seconds as usize,
            centis as usize,
        )
    }
    /// Rounded down. 6:59:00 is hour 6.
    pub fn get_hours(self) -> usize {
        self.get_parts().0
    }

    pub fn ampm_tostring(self) -> String {
        let (mut hours, minutes, seconds, _) = self.get_parts();
        let next_day = if hours >= 24 {
            let days = hours / 24;
            hours %= 24;
            format!(" (+{} days)", days)
        } else {
            "".to_string()
        };
        let suffix = if hours < 12 { "AM" } else { "PM" };
        if hours == 0 {
            hours = 12;
        } else if hours >= 24 {
            hours -= 24;
        } else if hours > 12 {
            hours -= 12;
        }

        format!(
            "{:02}:{:02}:{:02} {}{}",
            hours, minutes, seconds, suffix, next_day
        )
    }

    pub fn as_filename(self) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        format!(
            "{0:02}h{1:02}m{2:02}.{3:01}s",
            hours, minutes, seconds, remainder
        )
    }

    pub fn parse(string: &str) -> Result<Time> {
        let parts: Vec<&str> = string.split(':').collect();
        if parts.is_empty() {
            bail!("Time {}: no :'s", string);
        }

        let mut seconds = parts.last().unwrap().parse::<f64>()?;
        match parts.len() {
            1 => Ok(Time::seconds_since_midnight(seconds)),
            2 => {
                // They're really minutes
                seconds *= 60.0;
                seconds += 3600.0 * parts[0].parse::<f64>()?;
                Ok(Time::seconds_since_midnight(seconds))
            }
            3 => {
                seconds += 60.0 * parts[1].parse::<f64>()?;
                seconds += 3600.0 * parts[0].parse::<f64>()?;
                Ok(Time::seconds_since_midnight(seconds))
            }
            _ => bail!("Time {}: weird number of parts", string),
        }
    }

    // TODO These are a little weird, so don't operator overload yet
    pub fn percent_of(self, p: f64) -> Time {
        if !(0.0..=1.0).contains(&p) {
            panic!("Bad percent_of input: {}", p);
        }
        Time::seconds_since_midnight(self.0 * p)
    }

    pub fn to_percent(self, other: Time) -> f64 {
        self.0 / other.0
    }

    /// For RNG range generation. Don't abuse.
    pub fn inner_seconds(self) -> f64 {
        self.0
    }

    pub fn clamped_sub(self, dt: Duration) -> Time {
        Time::seconds_since_midnight((self.0 - dt.inner_seconds()).max(0.0))
    }

    pub fn round_seconds(self, s: f64) -> Time {
        Time::seconds_since_midnight(s * (self.0 / s).round())
    }
}

// 24-hour format by default
impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        write!(
            f,
            "{0:02}:{1:02}:{2:02}.{3:01}",
            hours, minutes, seconds, remainder
        )
    }
}

impl ops::Add<Duration> for Time {
    type Output = Time;

    fn add(self, other: Duration) -> Time {
        Time::seconds_since_midnight(self.0 + other.inner_seconds())
    }
}

impl ops::AddAssign<Duration> for Time {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl ops::Sub<Duration> for Time {
    type Output = Time;

    fn sub(self, other: Duration) -> Time {
        Time::seconds_since_midnight(self.0 - other.inner_seconds())
    }
}

impl ops::Sub for Time {
    type Output = Duration;

    fn sub(self, other: Time) -> Duration {
        Duration::seconds(self.0 - other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            Time::START_OF_DAY + Duration::seconds(42.3),
            Time::parse("42.3").unwrap()
        );
        assert_eq!(
            Time::START_OF_DAY + Duration::hours(7) + Duration::minutes(30),
            Time::parse("07:30").unwrap()
        );
        assert_eq!(
            Time::START_OF_DAY
                + Duration::hours(7)
                + Duration::minutes(30)
                + Duration::seconds(5.0),
            Time::parse("07:30:05").unwrap()
        );
    }

    #[test]
    fn get_hours() {
        assert_eq!((Time::START_OF_DAY + Duration::hours(6)).get_hours(), 6);
        assert_eq!(
            (Time::START_OF_DAY + Duration::hours(6) + Duration::seconds(1.0)).get_hours(),
            6
        );
        assert_eq!(
            (Time::START_OF_DAY + Duration::hours(6) + Duration::minutes(59)).get_hours(),
            6
        );
    }
}
