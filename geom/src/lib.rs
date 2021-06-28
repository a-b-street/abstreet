#![allow(clippy::new_without_default)]

#[macro_use]
extern crate anyhow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use crate::angle::Angle;
pub use crate::bounds::{Bounds, GPSBounds};
pub use crate::circle::Circle;
pub use crate::distance::Distance;
pub use crate::duration::Duration;
pub use crate::find_closest::FindClosest;
pub use crate::gps::LonLat;
pub use crate::line::{InfiniteLine, Line};
pub use crate::percent::Percent;
pub use crate::polygon::{Polygon, Triangle};
pub use crate::polyline::{ArrowCap, PolyLine};
pub use crate::pt::{HashablePt2D, Pt2D};
pub use crate::ring::Ring;
pub use crate::speed::Speed;
pub use crate::stats::{HgramValue, Histogram, Statistic};
pub use crate::time::Time;

mod angle;
mod bounds;
mod circle;
mod distance;
mod duration;
mod find_closest;
mod gps;
mod line;
mod percent;
mod polygon;
mod polyline;
mod pt;
mod ring;
mod speed;
mod stats;
mod time;

// About 0.4 inches... which is quite tiny on the scale of things. :)
pub const EPSILON_DIST: Distance = Distance::const_meters(0.01);

/// Reduce the precision of an f64. This helps ensure serialization is idempotent (everything is
/// exacly the same before and after saving/loading). Ideally we'd use some kind of proper
/// fixed-precision type instead of f64.
pub fn trim_f64(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Serializes a trimmed `f64` as an `i32` to save space.
fn serialize_f64<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    // So a trimmed f64's range becomes 2**31 / 10,000 =~ 214,000, which is plenty
    // We don't need to round() here; trim_f64 already handles that.
    let int = (x * 10_000.0) as i32;
    int.serialize(s)
}

/// Deserializes a trimmed `f64` from an `i32`.
fn deserialize_f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let x = <i32>::deserialize(d)?;
    Ok(x as f64 / 10_000.0)
}

/// Specifies how to stringify different geom objects.
#[derive(Clone, Serialize, Deserialize)]
pub struct UnitFmt {
    /// Round `Duration`s to a whole number of seconds.
    pub round_durations: bool,
    /// Display in metric; US imperial otherwise.
    pub metric: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CornerRadii {
    pub top_left: f64,
    pub top_right: f64,
    pub bottom_right: f64,
    pub bottom_left: f64,
}

impl CornerRadii {
    pub fn uniform(radius: f64) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub fn zero() -> Self {
        Self::uniform(0.0)
    }
}

impl std::convert::From<f64> for CornerRadii {
    fn from(uniform: f64) -> Self {
        Self::uniform(uniform)
    }
}

impl std::default::Default for CornerRadii {
    fn default() -> Self {
        Self::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    #[test]
    fn f64_trimming() {
        // Roundtrip a bunch of random f64's
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(42);
        for _ in 0..1_000 {
            let input = rng.gen_range(-214_000.00..214_000.0);
            let trimmed = trim_f64(input);
            println!("{} -> {}", input, trimmed);
            let json_roundtrip: f64 =
                abstutil::from_json(abstutil::to_json(&trimmed).as_bytes()).unwrap();
            let bincode_roundtrip: f64 =
                abstutil::from_binary(&abstutil::to_binary(&trimmed)).unwrap();
            assert_eq!(json_roundtrip, trimmed);
            assert_eq!(bincode_roundtrip, trimmed);
        }

        // Hardcode a particular case, where we can hand-verify that it trims to 4 decimal places
        let input = 1.2345678;
        let trimmed = trim_f64(input);
        let json_roundtrip: f64 =
            abstutil::from_json(abstutil::to_json(&trimmed).as_bytes()).unwrap();
        let bincode_roundtrip: f64 = abstutil::from_binary(&abstutil::to_binary(&trimmed)).unwrap();
        assert_eq!(json_roundtrip, 1.2346);
        assert_eq!(bincode_roundtrip, 1.2346);
    }
}
