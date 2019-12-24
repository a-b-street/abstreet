use crate::Duration;
use histogram::Histogram;
use serde_derive::{Deserialize, Serialize};

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

// TODO Generic histogram

#[derive(Clone)]
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

pub struct PercentageHistogram {
    count: usize,
    histogram: Histogram,
    min: f64,
    max: f64,
}

impl Default for PercentageHistogram {
    fn default() -> PercentageHistogram {
        PercentageHistogram {
            count: 0,
            histogram: Default::default(),
            min: 0.0,
            max: 0.0,
        }
    }
}

impl PercentageHistogram {
    pub fn new() -> PercentageHistogram {
        Default::default()
    }

    pub fn add(&mut self, p: f64) {
        if self.count == 0 {
            self.min = p;
            self.max = p;
        } else {
            self.min = self.min.min(p);
            self.max = self.max.max(p);
        }
        self.count += 1;
        self.histogram.increment((p * 1000.0) as u64).unwrap();
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
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        Some((self.histogram.percentile(p).unwrap() as f64) / 1000.0)
    }

    pub fn select(&self, stat: Statistic) -> String {
        assert_ne!(self.count, 0);
        let raw = match stat {
            Statistic::P50 => self.histogram.percentile(50.0).unwrap(),
            Statistic::P90 => self.histogram.percentile(90.0).unwrap(),
            Statistic::P99 => self.histogram.percentile(99.0).unwrap(),
            Statistic::Min => {
                return print_percentage(self.min);
            }
            Statistic::Mean => self.histogram.mean().unwrap(),
            Statistic::Max => {
                return print_percentage(self.max);
            }
        };
        print_percentage((raw as f64) / 1000.0)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    // Could implement PartialEq, but be a bit more clear how approximate this is
    pub fn seems_eq(&self, other: &PercentageHistogram) -> bool {
        self.describe() == other.describe()
    }
}

// TODO geom needs a Percentage type -- there are enough uses everywhere
fn print_percentage(p: f64) -> String {
    format!("{:.1}%", p * 100.0)
}
