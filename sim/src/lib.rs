// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate control;
#[macro_use]
extern crate derivative;
extern crate dimensioned;
extern crate ezgui;
extern crate geom;
extern crate graphics;
extern crate map_model;
extern crate multimap;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod draw_car;
mod draw_ped;
mod driving;
mod intersections;
mod parking;
mod sim;
mod walking;

use dimensioned::si;
use geom::{Angle, Pt2D};
use map_model::{Map, RoadID, TurnID};
pub use sim::{Benchmark, CarState, Sim};
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

pub const TIMESTEP: si::Second<f64> = si::Second {
    value_unsafe: 0.1,
    _marker: std::marker::PhantomData,
};
pub const SPEED_LIMIT: si::MeterPerSecond<f64> = si::MeterPerSecond {
    value_unsafe: 8.9408,
    _marker: std::marker::PhantomData,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick(u32);

impl Tick {
    pub fn zero() -> Tick {
        Tick(0)
    }

    pub fn as_time(&self) -> si::Second<f64> {
        (self.0 as f64) * TIMESTEP
    }

    pub fn increment(&mut self) {
        self.0 += 1;
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
        // TODO switch to minutes and hours when this gets big
        write!(f, "{0:.1}s", (self.0 as f64) * TIMESTEP.value_unsafe)
    }
}

// TODO this name isn't quite right :)
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub(crate) enum On {
    Road(RoadID),
    Turn(TurnID),
}

impl On {
    pub(crate) fn as_road(&self) -> RoadID {
        match self {
            &On::Road(id) => id,
            &On::Turn(_) => panic!("not a road"),
        }
    }

    pub(crate) fn as_turn(&self) -> TurnID {
        match self {
            &On::Turn(id) => id,
            &On::Road(_) => panic!("not a turn"),
        }
    }

    fn maybe_turn(&self) -> Option<TurnID> {
        match self {
            &On::Turn(id) => Some(id),
            &On::Road(_) => None,
        }
    }

    fn length(&self, map: &Map) -> si::Meter<f64> {
        match self {
            &On::Road(id) => map.get_r(id).length(),
            &On::Turn(id) => map.get_t(id).length(),
        }
    }

    fn dist_along(&self, dist: si::Meter<f64>, map: &Map) -> (Pt2D, Angle) {
        match self {
            &On::Road(id) => map.get_r(id).dist_along(dist),
            &On::Turn(id) => map.get_t(id).dist_along(dist),
        }
    }
}
