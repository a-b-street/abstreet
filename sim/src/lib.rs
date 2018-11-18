// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate backtrace;
extern crate control;
#[macro_use]
extern crate derivative;
extern crate dimensioned;
extern crate geom;
#[macro_use]
extern crate lazy_static;
// Order matters -- this must be before 'mod macros'
#[macro_use]
extern crate log;
extern crate map_model;
#[macro_use]
extern crate more_asserts;
extern crate multimap;
extern crate ordered_float;
#[macro_use]
extern crate pretty_assertions;
extern crate rand;
extern crate rayon;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

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

use abstutil::Cloneable;
pub use events::Event;
pub use instrument::save_backtraces;
pub use kinematics::VehicleType;
pub use make::{
    load, ABTest, ABTestResults, BorderSpawnOverTime, MapEdits, Neighborhood, NeighborhoodBuilder,
    OriginDestination, Scenario, SeedParkedCars, SimFlags, SpawnOverTime,
};
use map_model::{BuildingID, LaneID};
pub use physics::{Acceleration, Distance, Speed, Tick, Time, TIMESTEP};
pub use query::{Benchmark, ScoreSummary, SimStats, Summary};
pub use render::{CarState, DrawCarInput, DrawPedestrianInput};
pub use sim::Sim;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CarID(pub usize);

impl fmt::Display for CarID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CarID({0})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PedestrianID(pub usize);

impl fmt::Display for PedestrianID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PedestrianID({0})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RouteID(pub usize);

impl fmt::Display for RouteID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RouteID({0})", self.0)
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

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
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
impl Cloneable for Neighborhood {}
impl Cloneable for NeighborhoodBuilder {}
impl Cloneable for Scenario {}
impl Cloneable for Tick {}
impl Cloneable for MapEdits {}
impl Cloneable for ABTest {}
