mod car;
mod driving;
mod intersection;
mod queue;

pub use self::car::{Car, CarState};
pub use self::driving::DrivingSimState;
pub use self::intersection::IntersectionController;
pub use self::queue::Queue;
use geom::Distance;

pub const MIN_VEHICLE_LENGTH: Distance = Distance::const_meters(2.0);
pub const MAX_VEHICLE_LENGTH: Distance = Distance::const_meters(7.0);
pub const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);
