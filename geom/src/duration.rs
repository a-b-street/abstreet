use std::{cmp, ops};

use anyhow::Result;
use instant::Instant;
use serde::{Deserialize, Serialize};

use abstutil::elapsed_seconds;

use crate::{deserialize_f64, serialize_f64, trim_f64, Distance, Speed, UnitFmt};

/// A duration, in seconds. Can be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Duration(
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")] f64,
);

// By construction, Duration is a finite f64 with trimmed precision.
impl Eq for Duration {}

#[allow(clippy::derive_ord_xor_partial_ord)] // false positive
impl Ord for Duration {
    fn cmp(&self, other: &Duration) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Duration {
    pub const ZERO: Duration = Duration::const_seconds(0.0);
    pub const EPSILON: Duration = Duration::const_seconds(0.0001);

    /// Creates a duration in seconds.
    pub fn seconds(value: f64) -> Duration {
        if !value.is_finite() {
            panic!("Bad Duration {}", value);
        }

        Duration(trim_f64(value))
    }

    /// Creates a duration in minutes.
    pub fn minutes(mins: usize) -> Duration {
        Duration::seconds((mins as f64) * 60.0)
    }

    /// Creates a duration in hours.
    pub fn hours(hours: usize) -> Duration {
        Duration::seconds((hours as f64) * 3600.0)
    }

    /// Creates a duration in minutes.
    pub fn f64_minutes(mins: f64) -> Duration {
        Duration::seconds(mins * 60.0)
    }

    pub const fn const_seconds(value: f64) -> Duration {
        Duration(value)
    }

    pub(crate) fn to_u64(self) -> u64 {
        (self.0 / Duration::EPSILON.0) as u64
    }

    pub(crate) fn from_u64(x: u64) -> Duration {
        (x as f64) * Duration::EPSILON
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    /// Returns the duration in seconds. Prefer working in typesafe `Duration`s.
    // TODO Remove if possible.
    pub fn inner_seconds(self) -> f64 {
        self.0
    }

    /// Splits the duration into (hours, minutes, seconds, centiseconds).
    // TODO Could share some of this with Time -- the representations are the same
    fn get_parts(self) -> (usize, usize, usize, usize) {
        // Force positive
        let mut remainder = self.inner_seconds().abs();
        let hours = (remainder / 3600.0).floor();
        remainder -= hours * 3600.0;
        let minutes = (remainder / 60.0).floor();
        remainder -= minutes * 60.0;
        let seconds = remainder.floor();
        remainder -= seconds;
        let centis = (remainder / 0.01).round();

        (
            hours as usize,
            minutes as usize,
            seconds as usize,
            centis as usize,
        )
    }

    /// Parses a duration such as "3:00" to `Duration::minutes(3)`.
    // TODO This is NOT the inverse of Display!
    pub fn parse(string: &str) -> Result<Duration> {
        let parts: Vec<&str> = string.split(':').collect();
        if parts.is_empty() {
            bail!("Duration {}: no :'s", string);
        }

        let mut seconds: f64 = 0.0;
        if parts.last().unwrap().contains('.') {
            let last_parts: Vec<&str> = parts.last().unwrap().split('.').collect();
            if last_parts.len() != 2 {
                bail!("Duration {}: no . in last part", string);
            }
            seconds += last_parts[1].parse::<f64>()? / 10.0;
            seconds += last_parts[0].parse::<f64>()?;
        } else {
            seconds += parts.last().unwrap().parse::<f64>()?;
        }

        match parts.len() {
            1 => Ok(Duration::seconds(seconds)),
            2 => {
                seconds += 60.0 * parts[0].parse::<f64>()?;
                Ok(Duration::seconds(seconds))
            }
            3 => {
                seconds += 60.0 * parts[1].parse::<f64>()?;
                seconds += 3600.0 * parts[0].parse::<f64>()?;
                Ok(Duration::seconds(seconds))
            }
            _ => bail!("Duration {}: weird number of parts", string),
        }
    }

    /// If two durations are within this amount, they'll print as if they're the same.
    pub fn epsilon_eq(self, other: Duration) -> bool {
        let eps = Duration::seconds(0.1);
        match self.cmp(&other) {
            cmp::Ordering::Greater => self - other < eps,
            cmp::Ordering::Less => other - self < eps,
            cmp::Ordering::Equal => true,
        }
    }

    /// Returns the duration elapsed from this moment in real time.
    pub fn realtime_elapsed(since: Instant) -> Duration {
        Duration::seconds(elapsed_seconds(since))
    }

    /// Rounds a duration up to the nearest whole number multiple.
    pub fn round_up(self, multiple: Duration) -> Duration {
        let remainder = self % multiple;
        if remainder == Duration::ZERO {
            self
        } else {
            self + multiple - remainder
        }
    }

    /// Returns the duration as a number of minutes, rounded up.
    pub fn num_minutes_rounded_up(self) -> usize {
        let (hrs, mins, secs, rem) = self.get_parts();
        let mut result = mins + 60 * hrs;
        if secs != 0 || rem != 0 {
            result += 1;
        }
        result
    }

    // TODO Do something fancier? http://vis.stanford.edu/papers/tick-labels
    // TODO Unit test me
    /// Returns (rounded max, the boundaries)
    pub fn make_intervals_for_max(self, num_labels: usize) -> (Duration, Vec<Duration>) {
        // Example 1: 43 minutes, max 5 labels... raw_mins_per_interval is 8.6
        let raw_mins_per_interval = (self.num_minutes_rounded_up() as f64) / (num_labels as f64);
        // So then this rounded up to 10 minutes
        let mut mins_per_interval = Duration::seconds(60.0 * raw_mins_per_interval)
            .round_up(Duration::minutes(5))
            .num_minutes_rounded_up();

        // Example 2: 8 minutes, max 5 labels... raw_mins_per_interval is 1.6
        // If we're under 25 minutes, this is going to be weird.
        if self < (num_labels as f64) * Duration::minutes(5) {
            // rounded up to 5 mins? 1 min increments
            // up to 10? 2 min increments
            // up to 15? 3
            // up to 20? 4
            // then after that the normal behavior
            mins_per_interval = (self.round_up(Duration::minutes(5)) / (num_labels as f64))
                .num_minutes_rounded_up();
        }

        let max = (num_labels as f64) * Duration::minutes(mins_per_interval);
        let labels = (0..=num_labels)
            .map(|i| Duration::minutes(i * mins_per_interval))
            .collect();

        if max < self {
            panic!(
                "Wait max of {} with {} labels wound up with rounded max of {}",
                self, num_labels, max
            );
        }
        (max, labels)
    }

    /// Shows only the largest unit (hours, minute, seconds), rounded to `precision` decimal points.
    ///
    /// ```
    /// use geom::Duration;
    /// assert_eq!(Duration::seconds(3600.0).to_rounded_string(0), "1hr");
    /// assert_eq!(Duration::seconds(3600.0).to_rounded_string(1), "1.0hr");
    /// assert_eq!(Duration::seconds(7800.0).to_rounded_string(0), "2hr");
    /// assert_eq!(Duration::seconds(800.0).to_rounded_string(1), "13.3min");
    /// assert_eq!(Duration::seconds(-800.0).to_rounded_string(1), "-13.3min");
    /// assert_eq!(Duration::seconds(0.0).to_rounded_string(0), "0");
    /// assert_eq!(Duration::seconds(12.5).to_rounded_string(1), "12.5s");
    /// assert_eq!(Duration::seconds(12.5).to_rounded_string(2), "12.50s");
    /// ```
    pub fn to_rounded_string(self, precision: usize) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        if hours == 0 && minutes == 0 && seconds == 0 && remainder == 0 {
            return "0".to_string();
        }

        let sign = if self < Duration::ZERO { "-" } else { "" };

        let (whole, part, unit) = {
            if hours != 0 {
                let whole = hours as f64;
                let part = minutes as f64 / 60.0;
                let unit = "hr";
                (whole, part, unit)
            } else if minutes != 0 {
                let whole = minutes as f64;
                let part = seconds as f64 / 60.0;
                let unit = "min";
                (whole, part, unit)
            } else {
                let whole = seconds as f64;
                let part = remainder as f64 / 100.0;
                let unit = "s";
                (whole, part, unit)
            }
        };

        let number = format!("{:.1$}", whole + part, precision);
        return format!("{}{}{}", sign, number, unit);
    }

    /// Describes the duration according to formatting rules.
    pub fn to_string(self, fmt: &UnitFmt) -> String {
        let mut s = String::new();
        if self < Duration::ZERO {
            s = "-".to_string();
        }
        let (hours, minutes, seconds, remainder) = self.get_parts();
        if hours == 0 && minutes == 0 && seconds == 0 && remainder == 0 {
            return "0s".to_string();
        }

        if hours != 0 {
            s = format!("{}{}hr ", s, hours);
        }
        if minutes != 0 {
            s = format!("{}{}min ", s, minutes);
        }
        if remainder != 0 {
            if fmt.round_durations {
                s = format!("{}{}s", s, seconds);
            } else {
                s = format!("{}{}.{:01}s", s, seconds, remainder);
            }
        } else if seconds != 0 {
            s = format!("{}{}s", s, seconds);
        }
        // Trim trailing whitespace, in case we have non-zero hours/minutes, but zero seconds
        s.trim_end().to_string()
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            (*self).to_string(&UnitFmt {
                metric: false,
                round_durations: false
            })
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

impl ops::Div<f64> for Duration {
    type Output = Duration;

    fn div(self, other: f64) -> Duration {
        if other == 0.0 {
            panic!("Can't divide {} / {}", self, other);
        }
        Duration::seconds(self.0 / other)
    }
}

impl ops::Rem<Duration> for Duration {
    type Output = Duration;

    fn rem(self, other: Duration) -> Duration {
        Duration::seconds(self.0 % other.0)
    }
}

impl std::iter::Sum for Duration {
    fn sum<I>(iter: I) -> Duration
    where
        I: Iterator<Item = Duration>,
    {
        let mut sum = Duration::ZERO;
        for x in iter {
            sum += x;
        }
        sum
    }
}

impl Default for Duration {
    fn default() -> Duration {
        Duration::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_durations() {
        let dont_round = UnitFmt {
            metric: false,
            round_durations: false,
        };
        let round = UnitFmt {
            metric: false,
            round_durations: true,
        };

        assert_eq!("0s", Duration::ZERO.to_string(&dont_round));
        assert_eq!("0s", Duration::seconds(0.001).to_string(&dont_round));
        assert_eq!(
            "1min 30.12s",
            Duration::seconds(90.123).to_string(&dont_round)
        );
        assert_eq!("1min 30s", Duration::seconds(90.123).to_string(&round));
        assert_eq!(
            "2hr 33min 5s",
            (Duration::hours(2) + Duration::minutes(33) + Duration::seconds(5.0))
                .to_string(&dont_round)
        );
        assert_eq!("3hr", Duration::hours(3).to_string(&dont_round));
        assert_eq!("42min", Duration::minutes(42).to_string(&dont_round));
    }
}
