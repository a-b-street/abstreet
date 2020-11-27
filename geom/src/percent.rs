use std::fmt;

/// Most of the time, [0, 1]. But some callers may go outside this range.
#[derive(Clone, Copy, PartialEq)]
pub struct Percent(f64);

impl Percent {
    pub fn inner(self) -> f64 {
        self.0
    }

    pub fn int(x: usize) -> Percent {
        if x > 100 {
            panic!("Percent::int({}) too big", x);
        }
        Percent((x as f64) / 100.0)
    }

    pub fn of(x: usize, total: usize) -> Percent {
        Percent((x as f64) / (total as f64))
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.2}%", self.0 * 100.0)
    }
}
