use crate::{trim_f64, Distance, Speed, Time};
use histogram::Histogram;
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
    const EPSILON: Duration = Duration::const_seconds(0.0001);
    // This isn't the last possible time, but for UI control purposes, it'll do.
    pub const END_OF_DAY: Duration =
        Duration::const_seconds(59.9 + (59.0 * 60.0) + (23.0 * 3600.0));

    pub fn seconds(value: f64) -> Duration {
        if !value.is_finite() {
            panic!("Bad Duration {}", value);
        }

        Duration(trim_f64(value))
    }

    pub fn minutes(mins: usize) -> Duration {
        Duration::seconds((mins as f64) * 60.0)
    }

    pub fn f64_minutes(mins: f64) -> Duration {
        Duration::seconds(mins * 60.0)
    }

    pub const fn const_seconds(value: f64) -> Duration {
        Duration(value)
    }

    fn to_u64(self) -> u64 {
        (self.0 / Duration::EPSILON.0) as u64
    }

    fn from_u64(x: u64) -> Duration {
        (x as f64) * Duration::EPSILON
    }

    pub fn min(self, other: Duration) -> Duration {
        if self <= other {
            self
        } else {
            other
        }
    }

    pub fn max(self, other: Duration) -> Duration {
        if self >= other {
            self
        } else {
            other
        }
    }

    // TODO Remove if possible.
    pub fn inner_seconds(self) -> f64 {
        self.0
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
        let centis = (remainder / 0.1).floor();

        (
            hours as usize,
            minutes as usize,
            seconds as usize,
            centis as usize,
        )
    }

    pub fn minimal_tostring(self) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        let mut s = String::new();
        if hours != 0 {
            s.push_str(&format!("{}h", hours));
        }
        if hours != 0 || minutes != 0 {
            s.push_str(&format!("{}m", minutes));
        }
        if remainder != 0 {
            s.push_str(&format!("{}.{:01}s", seconds, remainder));
        } else {
            s.push_str(&format!("{}s", seconds));
        }
        s
    }

    pub fn ampm_tostring(self) -> String {
        let (mut hours, minutes, seconds, remainder) = self.get_parts();
        let suffix = if hours < 12 {
            "AM"
        } else if hours < 24 {
            "PM"
        } else {
            // Give up on the AM/PM distinction I guess. This shouldn't be used much.
            "(+1 day)"
        };
        if hours == 0 {
            hours = 12;
        } else if hours >= 24 {
            hours -= 24;
        } else if hours > 12 {
            hours -= 12;
        }

        format!(
            "{:02}:{:02}:{:02}.{:01} {}",
            hours, minutes, seconds, remainder, suffix
        )
    }

    pub fn as_filename(self) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        format!(
            "{0:02}h{1:02}m{2:02}.{3:01}s",
            hours, minutes, seconds, remainder
        )
    }

    pub fn parse(string: &str) -> Result<Duration, abstutil::Error> {
        let parts: Vec<&str> = string.split(':').collect();
        if parts.is_empty() {
            return Err(abstutil::Error::new(format!("Duration {}: no :'s", string)));
        }

        let mut seconds: f64 = 0.0;
        if parts.last().unwrap().contains('.') {
            let last_parts: Vec<&str> = parts.last().unwrap().split('.').collect();
            if last_parts.len() != 2 {
                return Err(abstutil::Error::new(format!(
                    "Duration {}: no . in last part",
                    string
                )));
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
            _ => Err(abstutil::Error::new(format!(
                "Duration {}: weird number of parts",
                string
            ))),
        }
    }

    // If two durations are within this amount, they'll print as if they're the same.
    pub fn epsilon_eq(self, other: Duration) -> bool {
        let eps = Duration::seconds(0.1);
        if self > other {
            self - other < eps
        } else if self < other {
            other - self < eps
        } else {
            true
        }
    }

    pub fn since_midnight(self) -> Time {
        Time::START_OF_DAY + self
    }

    // TODO During transition only
    pub fn tmp_as_time(self) -> Time {
        Time::seconds_since_midnight(self.inner_seconds())
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

impl ops::Rem<Duration> for Duration {
    type Output = Duration;

    fn rem(self, other: Duration) -> Duration {
        Duration::seconds(self.0 % other.0)
    }
}

pub struct DurationHistogram {
    count: usize,
    histogram: Histogram,
    min: Duration,
    max: Duration,
}

impl Default for DurationHistogram {
    fn default() -> DurationHistogram {
        DurationHistogram {
            count: 0,
            histogram: Default::default(),
            min: Duration::ZERO,
            max: Duration::ZERO,
        }
    }
}

impl DurationHistogram {
    pub fn new() -> DurationHistogram {
        Default::default()
    }

    pub fn add(&mut self, t: Duration) {
        if self.count == 0 {
            self.min = t;
            self.max = t;
        } else {
            self.min = self.min.min(t);
            self.max = self.max.max(t);
        }
        self.count += 1;
        self.histogram.increment(t.to_u64()).unwrap();
    }

    pub fn describe(&self) -> String {
        if self.count == 0 {
            return "no data yet".to_string();
        }

        format!(
            "{} count, 50%ile {}, 90%ile {}, 99%ile {}, min {}, mean {}, max {}",
            abstutil::prettyprint_usize(self.count),
            self.select(Statistic::P50).minimal_tostring(),
            self.select(Statistic::P90).minimal_tostring(),
            self.select(Statistic::P99).minimal_tostring(),
            self.select(Statistic::Min).minimal_tostring(),
            self.select(Statistic::Mean).minimal_tostring(),
            self.select(Statistic::Max).minimal_tostring(),
        )
    }

    // None if empty
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.count == 0 {
            return None;
        }
        Some(Duration::from_u64(self.histogram.percentile(p).unwrap()))
    }

    pub fn select(&self, stat: Statistic) -> Duration {
        assert_ne!(self.count, 0);
        let raw = match stat {
            Statistic::P50 => self.histogram.percentile(50.0).unwrap(),
            Statistic::P90 => self.histogram.percentile(90.0).unwrap(),
            Statistic::P99 => self.histogram.percentile(99.0).unwrap(),
            Statistic::Min => {
                return self.min;
            }
            Statistic::Mean => self.histogram.mean().unwrap(),
            Statistic::Max => {
                return self.max;
            }
        };
        Duration::from_u64(raw)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    // Could implement PartialEq, but be a bit more clear how approximate this is
    pub fn seems_eq(&self, other: &DurationHistogram) -> bool {
        self.describe() == other.describe()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Statistic {
    Min,
    Mean,
    P50,
    P90,
    P99,
    Max,
}

impl Statistic {
    pub fn all() -> Vec<Statistic> {
        vec![
            Statistic::Min,
            Statistic::Mean,
            Statistic::P50,
            Statistic::P90,
            Statistic::P99,
            Statistic::Max,
        ]
    }
}

impl std::fmt::Display for Statistic {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Statistic::Min => write!(f, "minimum"),
            Statistic::Mean => write!(f, "mean"),
            Statistic::P50 => write!(f, "50%ile"),
            Statistic::P90 => write!(f, "90%ile"),
            Statistic::P99 => write!(f, "99%ile"),
            Statistic::Max => write!(f, "maximum"),
        }
    }
}
