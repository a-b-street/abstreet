use abstutil::elapsed_seconds;
use std::time::Instant;

pub struct Cycler {
    start: Instant,
    period_s: f64,
}

impl Cycler {
    pub fn new(period_s: f64) -> Cycler {
        Cycler {
            start: Instant::now(),
            period_s,
        }
    }

    // Returns [0.0, 1.0], going up from 0 to 1 and back down to 0 over the specified period.
    pub fn value(&self) -> f64 {
        let result: f64 = (elapsed_seconds(self.start) % self.period_s) / self.period_s;
        assert!(result >= 0.0 && result < 1.0);
        if result >= 0.5 {
            1.0 - result
        } else {
            result * 2.0
        }
    }
}
