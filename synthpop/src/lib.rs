//! This crate describes a synthetic population that exist in a map. Currently each person's travel
//! behavior is modelled, but in the future, demographic and health attributes may be added.
//! There's a variety of ways to create these populations, scattered in other crates.
//!
//! Note that "scenario" is the term currently used to describe the population. This will be
//! renamed "soon."

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use map_model::PathConstraints;

pub use self::endpoint::TripEndpoint;
pub use self::external::{ExternalPerson, ExternalTrip, ExternalTripEndpoint, MapBorders};
pub use self::modifier::ScenarioModifier;
pub use self::scenario::{IndividTrip, PersonSpec, Scenario, TripPurpose};

mod endpoint;
mod external;
mod modifier;
mod scenario;

/// How does a trip primarily happen?
///
/// Note most trips are "multi-modal" -- somebody has to walk a bit before and after parking their
/// car.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum TripMode {
    Walk,
    Bike,
    Transit,
    Drive,
}

impl TripMode {
    pub fn all() -> Vec<TripMode> {
        vec![
            TripMode::Walk,
            TripMode::Bike,
            TripMode::Transit,
            TripMode::Drive,
        ]
    }

    pub fn verb(self) -> &'static str {
        match self {
            TripMode::Walk => "walk",
            TripMode::Bike => "bike",
            TripMode::Transit => "use transit",
            TripMode::Drive => "drive",
        }
    }

    // If I used "present participle" in a method name, I'd never live it down.
    pub fn ongoing_verb(self) -> &'static str {
        match self {
            TripMode::Walk => "walking",
            TripMode::Bike => "biking",
            TripMode::Transit => "using transit",
            TripMode::Drive => "driving",
        }
    }

    pub fn noun(self) -> &'static str {
        match self {
            TripMode::Walk => "Pedestrian",
            TripMode::Bike => "Bike",
            TripMode::Transit => "Bus",
            TripMode::Drive => "Car",
        }
    }

    pub fn to_constraints(self) -> PathConstraints {
        match self {
            TripMode::Walk => PathConstraints::Pedestrian,
            TripMode::Bike => PathConstraints::Bike,
            // TODO WRONG
            TripMode::Transit => PathConstraints::Bus,
            TripMode::Drive => PathConstraints::Car,
        }
    }

    pub fn from_constraints(c: PathConstraints) -> TripMode {
        match c {
            PathConstraints::Pedestrian => TripMode::Walk,
            PathConstraints::Bike => TripMode::Bike,
            // TODO The bijection breaks down... transit rider vs train vs bus...
            PathConstraints::Bus | PathConstraints::Train => TripMode::Transit,
            PathConstraints::Car => TripMode::Drive,
        }
    }
}

/// This is an ID used by Seattle soundcast. Originally it was preserved for debugging, but that
/// hasn't happened in a long time. Also the format is tied to Soundcast. Consider deleting /
/// changing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OrigPersonID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);
