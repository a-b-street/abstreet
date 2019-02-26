use geom::Duration;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};

pub const TIMESTEP: Duration = Duration::const_seconds(0.1);

// Represents a moment in time, not a duration/delta
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick(u32);

impl Tick {
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub fn zero() -> Tick {
        Tick(0)
    }

    pub fn from_minutes(mins: u32) -> Tick {
        Tick(60 * 10 * mins)
    }

    pub fn from_seconds(secs: u32) -> Tick {
        Tick(10 * secs)
    }

    pub fn testonly_from_raw(t: u32) -> Tick {
        Tick(t)
    }

    // TODO as_duration?
    pub fn as_time(self) -> Duration {
        TIMESTEP * f64::from(self.0)
    }

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }

    pub fn prev(self) -> Tick {
        Tick(self.0 - 1)
    }

    pub fn is_multiple_of(self, other: Tick) -> bool {
        self.0 % other.0 == 0
    }

    fn get_parts(self) -> (u32, u32, u32, u32) {
        // TODO hardcoding these to avoid floating point issues... urgh. :\
        let ticks_per_second = 10;
        let ticks_per_minute = 60 * ticks_per_second;
        let ticks_per_hour = 60 * ticks_per_minute;

        let hours = self.0 / ticks_per_hour;
        let mut remainder = self.0 % ticks_per_hour;
        let minutes = remainder / ticks_per_minute;
        remainder %= ticks_per_minute;
        let seconds = remainder / ticks_per_second;
        remainder %= ticks_per_second;

        (hours, minutes, seconds, remainder)
    }

    pub fn as_filename(self) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        format!(
            "{0:02}h{1:02}m{2:02}.{3}s",
            hours, minutes, seconds, remainder
        )
    }

    // TODO options for sampling normal distribution
    pub fn uniform(start: Duration, stop: Duration, rng: &mut XorShiftRng) -> Tick {
        assert!(start < stop);
        Tick(rng.gen_range((start / TIMESTEP) as u32, (stop / TIMESTEP) as u32))
    }
}

impl std::ops::Add<Duration> for Tick {
    type Output = Tick;

    fn add(self, other: Duration) -> Tick {
        let ticks = other / TIMESTEP;
        // TODO check that there's no remainder!
        Tick(self.0 + (ticks as u32))
    }
}

impl std::ops::AddAssign<Tick> for Tick {
    fn add_assign(&mut self, other: Tick) {
        *self = Tick(self.0 + other.0)
    }
}

impl std::ops::Sub for Tick {
    type Output = Tick;

    fn sub(self, other: Tick) -> Tick {
        Tick(self.0 - other.0)
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        write!(
            f,
            "{0:02}:{1:02}:{2:02}.{3}",
            hours, minutes, seconds, remainder
        )
    }
}
