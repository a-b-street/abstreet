mod angle;
mod bounds;
mod circle;
mod find_closest;
mod gps;
mod line;
mod polygon;
mod polyline;
mod pt;
mod units;

pub use crate::angle::Angle;
pub use crate::bounds::{Bounds, GPSBounds};
pub use crate::circle::Circle;
pub use crate::find_closest::FindClosest;
pub use crate::gps::LonLat;
pub use crate::line::{InfiniteLine, Line};
pub use crate::polygon::{Polygon, Triangle};
pub use crate::polyline::PolyLine;
pub use crate::pt::{HashablePt2D, Pt2D};
pub use crate::units::{Acceleration, Distance, Duration, Speed};

// About 0.4 inches... which is quite tiny on the scale of things. :)
pub const EPSILON_DIST: Distance = Distance::const_meters(0.01);

pub(crate) fn trim_f64(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

impl abstutil::Cloneable for Duration {}
