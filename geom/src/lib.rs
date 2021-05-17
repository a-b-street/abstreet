#![allow(clippy::new_without_default)]

#[macro_use]
extern crate anyhow;

use serde::{Deserialize, Serialize};

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
