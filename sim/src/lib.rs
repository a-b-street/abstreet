#[macro_use]
mod macros;

mod driving;
mod events;
//mod fsm;
mod helpers;
mod instrument;
mod intersections;
// TODO pub only for tests...
pub mod kinematics;
mod make;
mod parking;
mod physics;
mod query;
mod render;
mod router;
mod scheduler;
mod sim;
mod spawn;
mod transit;
mod trips;
mod view;
mod walking;

pub use crate::events::Event;
pub use crate::instrument::save_backtraces;
pub use crate::kinematics::VehicleType;
pub use crate::make::{
    load, ABTest, ABTestResults, BorderSpawnOverTime, OriginDestination, Scenario, SeedParkedCars,
    SimFlags, SpawnOverTime,
};
pub use crate::physics::{Tick, TIMESTEP};
pub use crate::query::{Benchmark, ScoreSummary, SimStats, Summary};
pub use crate::render::{CarState, DrawCarInput, DrawPedestrianInput, GetDrawAgents};
pub use crate::sim::Sim;
use abstutil::Cloneable;
use map_model::{BuildingID, LaneID};
use serde_derive::{Deserialize, Serialize};
use std::fmt;

// The VehicleType is only used for convenient debugging. The numeric ID itself must be sufficient.
// TODO Implement Eq, Hash, Ord manually to guarantee this.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CarID(pub usize, VehicleType);

impl CarID {
    pub fn tmp_new(idx: usize, vt: VehicleType) -> CarID {
        CarID(idx, vt)
    }
}

impl fmt::Display for CarID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CarID({0} -- {1})",
            self.0,
            match self.1 {
                VehicleType::Car => "car",
                VehicleType::Bus => "bus",
                VehicleType::Bike => "bike",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PedestrianID(pub usize);

impl fmt::Display for PedestrianID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PedestrianID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
pub enum AgentID {
    Car(CarID),
    Pedestrian(PedestrianID),
}

impl AgentID {
    pub fn as_car(self) -> CarID {
        match self {
            AgentID::Car(id) => id,
            _ => panic!("Not a CarID: {:?}", self),
        }
    }
}

impl fmt::Display for AgentID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentID::Car(id) => write!(f, "AgentID({})", id),
            AgentID::Pedestrian(id) => write!(f, "AgentID({})", id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TripID(pub usize);

impl fmt::Display for TripID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TripID({0})", self.0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub car: CarID,
    pub spot: ParkingSpot,
    pub vehicle: kinematics::Vehicle,
    pub owner: Option<BuildingID>,
}

impl ParkedCar {
    pub fn new(
        car: CarID,
        spot: ParkingSpot,
        vehicle: kinematics::Vehicle,
        owner: Option<BuildingID>,
    ) -> ParkedCar {
        assert_eq!(vehicle.vehicle_type, VehicleType::Car);
        ParkedCar {
            car,
            spot,
            vehicle,
            owner,
        }
    }
}

// We have to do this in the crate where these types are defined. Bit annoying, since it's really
// kind of an ezgui concept.
impl Cloneable for Scenario {}
impl Cloneable for Tick {}
impl Cloneable for ABTest {}
