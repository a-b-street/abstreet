// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CarID(pub usize);

use std;
pub const TIMESTEP: si::Second<f64> = si::Second {
    value_unsafe: 0.1,
    _marker: std::marker::PhantomData,
};
pub const SPEED_LIMIT: si::MeterPerSecond<f64> = si::MeterPerSecond {
    value_unsafe: 8.9408,
    _marker: std::marker::PhantomData,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick(u32);

impl Tick {
    pub fn zero() -> Tick {
        Tick(0)
    }

    pub fn as_time(&self) -> si::Second<f64> {
        (self.0 as f64) * TIMESTEP
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

use std::fmt;
use std::ops::Sub;

impl Sub for Tick {
    type Output = Tick;

    fn sub(self, other: Tick) -> Tick {
        Tick(self.0 - other.0)
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO switch to minutes and hours when this gets big
        write!(f, "{0:.1}s", (self.0 as f64) * TIMESTEP.value_unsafe)
    }
}
