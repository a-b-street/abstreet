use dimensioned::si;
use lazy_static::lazy_static;
use rand::{Rng, XorShiftRng};
use regex::Regex;
use serde_derive::{Deserialize, Serialize};

pub const TIMESTEP: Time = si::Second {
    value_unsafe: 0.1,
    _marker: std::marker::PhantomData,
};

// TODO Don't just alias types; assert that time, dist, and speed are always positive
pub type Time = si::Second<f64>;
pub type Distance = si::Meter<f64>;
pub type Speed = si::MeterPerSecond<f64>;
pub type Acceleration = si::MeterPerSecond2<f64>;

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

    pub fn from_minutes(secs: u32) -> Tick {
        Tick(60 * 10 * secs)
    }

    pub fn from_seconds(secs: u32) -> Tick {
        Tick(10 * secs)
    }

    pub fn testonly_from_raw(t: u32) -> Tick {
        Tick(t)
    }

    // TODO Why have these two forms? Consolidate
    pub fn parse(string: &str) -> Option<Tick> {
        let parts: Vec<&str> = string.split(':').collect();
        if parts.is_empty() {
            return None;
        }

        let mut ticks: u32 = 0;
        if parts.last().unwrap().contains('.') {
            let last_parts: Vec<&str> = parts.last().unwrap().split('.').collect();
            if last_parts.len() != 2 {
                return None;
            }
            ticks += u32::from_str_radix(last_parts[1], 10).ok()?;
            ticks += 10 * u32::from_str_radix(last_parts[0], 10).ok()?;
        } else {
            ticks += 10 * u32::from_str_radix(parts.last().unwrap(), 10).ok()?;
        }

        match parts.len() {
            1 => Some(Tick(ticks)),
            2 => {
                ticks += 60 * 10 * u32::from_str_radix(parts[0], 10).ok()?;
                Some(Tick(ticks))
            }
            3 => {
                ticks += 60 * 10 * u32::from_str_radix(parts[1], 10).ok()?;
                ticks += 60 * 60 * 10 * u32::from_str_radix(parts[0], 10).ok()?;
                Some(Tick(ticks))
            }
            _ => None,
        }
    }

    pub fn parse_filename(string: &str) -> Option<Tick> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"(\d+)h(\d+)m(\d+)\.(\d+)s").unwrap();
        }
        let caps = RE.captures(string)?;
        let hours = 60 * 60 * 10 * u32::from_str_radix(&caps[1], 10).ok()?;
        let minutes = 60 * 10 * u32::from_str_radix(&caps[2], 10).ok()?;
        let seconds = 10 * u32::from_str_radix(&caps[3], 10).ok()?;
        let ms = u32::from_str_radix(&caps[4], 10).ok()?;

        Some(Tick(hours + minutes + seconds + ms))
    }

    pub fn as_time(self) -> Time {
        f64::from(self.0) * TIMESTEP
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
    pub fn uniform(start: Tick, stop: Tick, rng: &mut XorShiftRng) -> Tick {
        assert!(start < stop);
        Tick(rng.gen_range(start.0, stop.0))
    }
}

impl std::ops::Add<Time> for Tick {
    type Output = Tick;

    fn add(self, other: Time) -> Tick {
        let ticks = other.value_unsafe / TIMESTEP.value_unsafe;
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
