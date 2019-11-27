use crate::{trim_f64, Distance, Speed};
use abstutil::elapsed_seconds;
use histogram::Histogram;
use serde_derive::{Deserialize, Serialize};
use std::time::Instant;
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

    // TODO Remove if possible.
    pub fn inner_seconds(self) -> f64 {
        self.0
    }

    // TODO Could share some of this with Time -- the representations are the same
    // (hours, minutes, seconds, centiseconds)
    fn get_parts(self) -> (usize, usize, usize, usize) {
        // Force positive
        let mut remainder = self.inner_seconds().abs();
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

    // TODO This is NOT the inverse of Display!
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

    pub fn realtime_elapsed(since: Instant) -> Duration {
        Duration::seconds(elapsed_seconds(since))
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if *self < Duration::ZERO {
            write!(f, "-")?;
        }
        let (hours, minutes, seconds, remainder) = self.get_parts();
        if hours != 0 {
            write!(f, "{}h", hours)?;
        }
        if hours != 0 || minutes != 0 {
            write!(f, "{}m", minutes)?;
        }
        if remainder != 0 {
            write!(f, "{}.{:01}s", seconds, remainder)
        } else {
            write!(f, "{}s", seconds)
        }
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
            self.select(Statistic::P50),
            self.select(Statistic::P90),
            self.select(Statistic::P99),
            self.select(Statistic::Min),
            self.select(Statistic::Mean),
            self.select(Statistic::Max),
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
