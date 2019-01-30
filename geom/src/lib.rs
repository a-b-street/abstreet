mod angle;
mod circle;
mod gps;
mod line;
mod polygon;
mod polyline;
mod pt;
mod units;

pub use crate::angle::Angle;
pub use crate::circle::Circle;
pub use crate::gps::{GPSBounds, LonLat};
pub use crate::line::{InfiniteLine, Line};
pub use crate::polygon::{Polygon, Triangle};
pub use crate::polyline::PolyLine;
pub use crate::pt::{Bounds, HashablePt2D, Pt2D};
pub use crate::units::{Acceleration, Distance, Duration, Speed};

// About 0.4 inches... which is quite tiny on the scale of things. :)
pub const EPSILON_DIST: Distance = Distance::const_meters(0.01);
