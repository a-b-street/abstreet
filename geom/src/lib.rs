#[macro_use]
extern crate anyhow;

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
#[derive(Clone)]
pub struct UnitFmt {
    /// Round `Duration`s to a whole number of seconds.
    pub round_durations: bool,
    /// Display in metric; US imperial otherwise.
    pub metric: bool,
}
