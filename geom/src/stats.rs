use crate::Duration;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

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

pub trait HgramValue<T>: Copy + std::cmp::Ord + std::fmt::Display {
    fn zero() -> T;
    fn to_u64(self) -> u64;
    fn from_u64(x: u64) -> T;
}

impl HgramValue<Duration> for Duration {
    fn zero() -> Duration {
        Duration::ZERO
    }
    fn to_u64(self) -> u64 {
        self.to_u64()
    }
    fn from_u64(x: u64) -> Duration {
        Duration::from_u64(x)
    }
}

impl HgramValue<u16> for u16 {
    fn zero() -> u16 {
        0
    }
    fn to_u64(self) -> u64 {
        self as u64
    }
    fn from_u64(x: u64) -> u16 {
        u16::try_from(x).unwrap()
    }
}

impl HgramValue<usize> for usize {
    fn zero() -> usize {
        0
    }
    fn to_u64(self) -> u64 {
        self as u64
    }
    fn from_u64(x: u64) -> usize {
        x as usize
    }
}

#[derive(Clone)]
pub struct Histogram<T: HgramValue<T>> {
    count: usize,
    histogram: histogram::Histogram,
    min: T,
    max: T,
}

impl<T: HgramValue<T>> Default for Histogram<T> {
    fn default() -> Histogram<T> {
        Histogram {
            count: 0,
            histogram: Default::default(),
            min: T::zero(),
            max: T::zero(),
        }
    }
}

impl<T: HgramValue<T>> Histogram<T> {
    pub fn new() -> Histogram<T> {
        Default::default()
    }

    pub fn add(&mut self, x: T) {
        if self.count == 0 {
            self.min = x;
            self.max = x;
        } else {
            self.min = self.min.min(x);
            self.max = self.max.max(x);
        }
        self.count += 1;
        self.histogram.increment(x.to_u64()).unwrap();
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
    pub fn percentile(&self, p: f64) -> Option<T> {
        if self.count == 0 {
            return None;
        }
        Some(T::from_u64(self.histogram.percentile(p).unwrap()))
    }

    pub fn select(&self, stat: Statistic) -> T {
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
        T::from_u64(raw)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    // Could implement PartialEq, but be a bit more clear how approximate this is
    pub fn seems_eq(&self, other: &Histogram<T>) -> bool {
        self.describe() == other.describe()
    }
}
