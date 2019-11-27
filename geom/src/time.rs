use crate::trim_f64;
use serde_derive::{Deserialize, Serialize};
use std::cmp;

// In seconds since midnight. Can't be negative.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Time(f64);

// By construction, Time is a finite f64 with trimmed precision.
impl Eq for Time {}
impl Ord for Time {
    fn cmp(&self, other: &Time) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Time {
    pub const START_OF_DAY: Time = Time(0.0);
    // This isn't the last possible time, but for UI control purposes, it'll do.
    pub const END_OF_DAY: Time = Time(59.9 + (59.0 * 60.0) + (23.0 * 3600.0));

    pub fn seconds_since_midnight(value: f64) -> Time {
        if !value.is_finite() || value < 0.0 {
            panic!("Bad Time {}", value);
        }

        Time(trim_f64(value))
    }

    // (hours, minutes, seconds, centiseconds)
    fn get_parts(self) -> (usize, usize, usize, usize) {
        let mut remainder = self.0;
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

    pub fn parse(string: &str) -> Result<Time, abstutil::Error> {
        let parts: Vec<&str> = string.split(':').collect();
        if parts.is_empty() {
            return Err(abstutil::Error::new(format!("Time {}: no :'s", string)));
        }

        let mut seconds: f64 = 0.0;
        if parts.last().unwrap().contains('.') {
            let last_parts: Vec<&str> = parts.last().unwrap().split('.').collect();
            if last_parts.len() != 2 {
                return Err(abstutil::Error::new(format!(
                    "Time {}: no . in last part",
                    string
                )));
            }
            seconds += last_parts[1].parse::<f64>()? / 10.0;
            seconds += last_parts[0].parse::<f64>()?;
        } else {
            seconds += parts.last().unwrap().parse::<f64>()?;
        }

        match parts.len() {
            1 => Ok(Time::seconds_since_midnight(seconds)),
            2 => {
                seconds += 60.0 * parts[0].parse::<f64>()?;
                Ok(Time::seconds_since_midnight(seconds))
            }
            3 => {
                seconds += 60.0 * parts[1].parse::<f64>()?;
                seconds += 3600.0 * parts[0].parse::<f64>()?;
                Ok(Time::seconds_since_midnight(seconds))
            }
            _ => Err(abstutil::Error::new(format!(
                "Time {}: weird number of parts",
                string
            ))),
        }
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        write!(
            f,
            "{0:02}:{1:02}:{2:02}.{3:01}",
            hours, minutes, seconds, remainder
        )
    }
}
