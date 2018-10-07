use abstutil::elapsed_seconds;
use std::f64::consts::PI;
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
        // Ranges from -1 to 1
        let cosine = (elapsed_seconds(self.start) * (2.0 * PI) / self.period_s).cos();
        let result = (cosine + 1.0) / 2.0;
        assert!(result >= 0.0 && result < 1.0);
        result
    }
}
