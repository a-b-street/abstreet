mod car;
mod driving;
mod intersection;
mod parking;
mod queue;
mod router;
mod sim;

pub use self::car::{Car, CarState, TimeInterval};
pub use self::driving::DrivingSimState;
pub use self::intersection::IntersectionController;
pub use self::parking::ParkingSimState;
pub use self::queue::Queue;
pub use self::router::{ActionAtEnd, Router};
pub use self::sim::Sim;
use ::sim::{CarID, VehicleType};
use geom::{Distance, Speed};
use map_model::{BuildingID, LaneID};
use serde_derive::{Deserialize, Serialize};

pub const MIN_VEHICLE_LENGTH: Distance = Distance::const_meters(2.0);
pub const MAX_VEHICLE_LENGTH: Distance = Distance::const_meters(7.0);
pub const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: CarID,
    pub vehicle_type: VehicleType,

    pub length: Distance,
    pub max_speed: Option<Speed>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParkingSpot {
    pub lane: LaneID,
    pub idx: usize,
}

impl ParkingSpot {
    pub fn new(lane: LaneID, idx: usize) -> ParkingSpot {
        ParkingSpot { lane, idx }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ParkedCar {
    pub vehicle: Vehicle,
    pub spot: ParkingSpot,
    pub owner: Option<BuildingID>,
}

impl ParkedCar {
    pub fn new(vehicle: Vehicle, spot: ParkingSpot, owner: Option<BuildingID>) -> ParkedCar {
        assert_eq!(vehicle.vehicle_type, VehicleType::Car);
        ParkedCar {
            vehicle,
            spot,
            owner,
        }
    }
}
