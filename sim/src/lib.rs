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
mod helpers;
mod instrument;
mod intersections;
// TODO pub only for tests...
pub mod kinematics;
mod make;
mod parking;
mod query;
mod router;
mod sim;
mod spawn;
mod transit;
mod trips;
mod view;
mod walking;

use abstutil::Cloneable;
use dimensioned::si;
pub use events::Event;
use geom::{Angle, Pt2D};
pub use instrument::save_backtraces;
pub use make::{
    load, ABTest, ABTestResults, BorderSpawnOverTime, MapEdits, Neighborhood, NeighborhoodBuilder,
    OriginDestination, Scenario, SeedParkedCars, SimFlags, SpawnOverTime,
};
use map_model::{BuildingID, LaneID, Trace, TurnID};
pub use query::{Benchmark, ScoreSummary, SimStats, Summary};
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

pub const TIMESTEP: Time = si::Second {
    value_unsafe: 0.1,
    _marker: std::marker::PhantomData,
};

// Represents a moment in time, not a duration/delta
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick(u32);

impl Tick {
    pub fn zero() -> Tick {
        Tick(0)
    }

    pub fn from_minutes(secs: u32) -> Tick {
        Tick(60 * 10 * secs)
    }

    pub fn from_seconds(secs: u32) -> Tick {
        Tick(10 * secs)
    }

    pub fn parse(string: &str) -> Option<Tick> {
        let parts: Vec<&str> = string.split(":").collect();
        if parts.is_empty() {
            return None;
        }

        let mut ticks: u32 = 0;
        if parts.last().unwrap().contains(".") {
            let last_parts: Vec<&str> = parts.last().unwrap().split(".").collect();
            if last_parts.len() != 2 {
                return None;
            }
            ticks += u32::from_str_radix(last_parts[1], 10).ok()?;
            ticks += 10 * u32::from_str_radix(last_parts[0], 10).ok()?;
        } else {
            ticks += 10 * u32::from_str_radix(parts.last().unwrap(), 10).ok()?;
        }

        match parts.len() {
            1 => Some(Tick(ticks)),
            2 => {
                ticks += 60 * 10 * u32::from_str_radix(parts[0], 10).ok()?;
                Some(Tick(ticks))
            }
            3 => {
                ticks += 60 * 10 * u32::from_str_radix(parts[1], 10).ok()?;
                ticks += 60 * 60 * 10 * u32::from_str_radix(parts[0], 10).ok()?;
                Some(Tick(ticks))
            }
            _ => None,
        }
    }

    pub fn as_time(&self) -> Time {
        (self.0 as f64) * TIMESTEP
    }

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }

    pub fn is_multiple_of(&self, other: Tick) -> bool {
        self.0 % other.0 == 0
    }

    fn get_parts(&self) -> (u32, u32, u32, u32) {
        // TODO hardcoding these to avoid floating point issues... urgh. :\
        let ticks_per_second = 10;
        let ticks_per_minute = 60 * ticks_per_second;
        let ticks_per_hour = 60 * ticks_per_minute;

        let hours = self.0 / ticks_per_hour;
        let mut remainder = self.0 % ticks_per_hour;
        let minutes = remainder / ticks_per_minute;
        remainder = remainder % ticks_per_minute;
        let seconds = remainder / ticks_per_second;
        remainder = remainder % ticks_per_second;

        (hours, minutes, seconds, remainder)
    }

    pub fn as_filename(&self) -> String {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        format!(
            "{0:02}h{1:02}m{2:02}.{3}s",
            hours, minutes, seconds, remainder
        )
    }
}

impl std::ops::Add<Time> for Tick {
    type Output = Tick;

    fn add(self, other: Time) -> Tick {
        let ticks = other.value_unsafe / TIMESTEP.value_unsafe;
        // TODO check that there's no remainder!
        Tick(self.0 + (ticks as u32))
    }
}

impl std::ops::AddAssign<Tick> for Tick {
    fn add_assign(&mut self, other: Tick) {
        *self = Tick(self.0 + other.0)
    }
}

impl std::ops::Sub for Tick {
    type Output = Tick;

    fn sub(self, other: Tick) -> Tick {
        Tick(self.0 - other.0)
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (hours, minutes, seconds, remainder) = self.get_parts();
        write!(
            f,
            "{0:02}:{1:02}:{2:02}.{3}",
            hours, minutes, seconds, remainder
        )
    }
}

#[test]
fn time_parsing() {
    assert_eq!(Tick::parse("2.3"), Some(Tick(23)));
    assert_eq!(Tick::parse("02.3"), Some(Tick(23)));
    assert_eq!(Tick::parse("00:00:02.3"), Some(Tick(23)));

    assert_eq!(Tick::parse("00:02:03.5"), Some(Tick(35 + 1200)));
    assert_eq!(Tick::parse("01:02:03.5"), Some(Tick(35 + 1200 + 36000)));
}

// TODO Don't just alias types; assert that time, dist, and speed are always positive
pub type Time = si::Second<f64>;
pub type Distance = si::Meter<f64>;
pub type Speed = si::MeterPerSecond<f64>;
pub type Acceleration = si::MeterPerSecond2<f64>;

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

// Intermediate structures so that sim and editor crates don't have a cyclic dependency.
pub struct DrawPedestrianInput {
    pub id: PedestrianID,
    pub pos: Pt2D,
    pub waiting_for_turn: Option<TurnID>,
}

pub struct DrawCarInput {
    pub id: CarID,
    pub vehicle_length: Distance,
    pub waiting_for_turn: Option<TurnID>,
    pub front: Pt2D,
    pub angle: Angle,
    pub stopping_trace: Option<Trace>,
    pub state: CarState,
}

#[derive(PartialEq, Eq)]
pub enum CarState {
    Moving,
    Stuck,
    Parked,
    Debug,
}

// We have to do this in the crate where these types are defined. Bit annoying, since it's really
// kind of an ezgui concept.
impl Cloneable for Neighborhood {}
impl Cloneable for NeighborhoodBuilder {}
impl Cloneable for Scenario {}
impl Cloneable for Tick {}
impl Cloneable for MapEdits {}
impl Cloneable for ABTest {}
