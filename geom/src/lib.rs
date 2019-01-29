mod angle;
mod circle;
mod gps;
mod line;
mod polygon;
mod polyline;
mod pt;

pub use crate::angle::Angle;
pub use crate::circle::Circle;
pub use crate::gps::{GPSBounds, LonLat};
pub use crate::line::{InfiniteLine, Line};
pub use crate::polygon::{Polygon, Triangle};
pub use crate::polyline::PolyLine;
pub use crate::pt::{Bounds, HashablePt2D, Pt2D};
use dimensioned::si;
use std::marker;

// About 0.4 inches... which is quite tiny on the scale of things. :)
pub const EPSILON_DIST: si::Meter<f64> = si::Meter {
    value_unsafe: 0.01,
    _marker: marker::PhantomData,
};
