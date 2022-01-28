use std::collections::BTreeSet;
use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::{CityName, MapName};
use abstutil::prettyprint_usize;
use geom::Time;
use map_model::Map;

use crate::{OrigPersonID, TripEndpoint, TripMode};

/// A Scenario describes all the input to a simulation. Usually a scenario covers one day.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: MapName,

    pub people: Vec<PersonSpec>,
    /// None means seed all buses. Otherwise the route name must be present here.
    pub only_seed_buses: Option<BTreeSet<String>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PersonSpec {
    /// Just used for debugging
    pub orig_id: Option<OrigPersonID>,
    /// There must be continuity between trips: each trip starts at the destination of the previous
    /// trip. In the case of borders, the outbound and inbound border may be different. This means
    /// that there was some sort of "remote" trip happening outside the map that we don't simulate.
    pub trips: Vec<IndividTrip>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IndividTrip {
    pub depart: Time,
    pub origin: TripEndpoint,
    pub destination: TripEndpoint,
    pub mode: TripMode,
    pub purpose: TripPurpose,
    pub cancelled: bool,
    /// Did a ScenarioModifier affect this?
    pub modified: bool,
}

impl IndividTrip {
    pub fn new(
        depart: Time,
        purpose: TripPurpose,
        origin: TripEndpoint,
        destination: TripEndpoint,
        mode: TripMode,
    ) -> IndividTrip {
        IndividTrip {
            depart,
            origin,
            destination,
            mode,
            purpose,
            cancelled: false,
            modified: false,
        }
    }
}

/// Lifted from Seattle's Soundcast model, but seems general enough to use anyhere.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum TripPurpose {
    Home,
    Work,
    School,
    Escort,
    PersonalBusiness,
    Shopping,
    Meal,
    Social,
    Recreation,
    Medical,
    ParkAndRideTransfer,
}

impl fmt::Display for TripPurpose {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TripPurpose::Home => "home",
                TripPurpose::Work => "work",
                TripPurpose::School => "school",
                // Is this like a parent escorting a child to school?
                TripPurpose::Escort => "escort",
                TripPurpose::PersonalBusiness => "personal business",
                TripPurpose::Shopping => "shopping",
                TripPurpose::Meal => "eating",
                TripPurpose::Social => "social",
                TripPurpose::Recreation => "recreation",
                TripPurpose::Medical => "medical",
                TripPurpose::ParkAndRideTransfer => "park-and-ride transfer",
            }
        )
    }
}

impl Scenario {
    pub fn save(&self) {
        abstio::write_binary(
            abstio::path_scenario(&self.map_name, &self.scenario_name),
            self,
        );
    }

    pub fn empty(map: &Map, name: &str) -> Scenario {
        Scenario {
            scenario_name: name.to_string(),
            map_name: map.get_name().clone(),
            people: Vec::new(),
            only_seed_buses: Some(BTreeSet::new()),
        }
    }

    pub fn remove_weird_schedules(mut self) -> Scenario {
        let orig = self.people.len();
        self.people.retain(|person| match person.check_schedule() {
            Ok(()) => true,
            Err(err) => {
                println!("{}", err);
                false
            }
        });
        warn!(
            "{} of {} people have nonsense schedules",
            prettyprint_usize(orig - self.people.len()),
            prettyprint_usize(orig)
        );
        self
    }

    pub fn all_trips(&self) -> impl Iterator<Item = &IndividTrip> {
        self.people.iter().flat_map(|p| p.trips.iter())
    }

    pub fn default_scenario_for_map(name: &MapName) -> String {
        if name.city == CityName::seattle()
            && abstio::file_exists(abstio::path_scenario(name, "weekday"))
        {
            return "weekday".to_string();
        }
        if name.city.country == "gb" {
            for x in ["background", "base_with_bg"] {
                if abstio::file_exists(abstio::path_scenario(name, x)) {
                    return x.to_string();
                }
            }
        }
        // Dynamically generated -- arguably this is an absence of a default scenario
        "home_to_work".to_string()
    }
}

impl PersonSpec {
    /// Verify that a person's trips make sense
    fn check_schedule(&self) -> Result<()> {
        if self.trips.is_empty() {
            bail!("Person ({:?}) has no trips at all", self.orig_id);
        }

        for pair in self.trips.windows(2) {
            if pair[0].depart >= pair[1].depart {
                bail!(
                    "Person ({:?}) starts two trips in the wrong order: {} then {}",
                    self.orig_id,
                    pair[0].depart,
                    pair[1].depart
                );
            }

            if pair[0].destination != pair[1].origin {
                // Exiting one border and re-entering another is fine
                if matches!(pair[0].destination, TripEndpoint::Border(_))
                    && matches!(pair[1].origin, TripEndpoint::Border(_))
                {
                    continue;
                }
                bail!(
                    "Person ({:?}) warps from {:?} to {:?} during adjacent trips",
                    self.orig_id,
                    pair[0].destination,
                    pair[1].origin
                );
            }
        }

        for trip in &self.trips {
            if trip.origin == trip.destination {
                bail!(
                    "Person ({:?}) has a trip from/to the same place: {:?}",
                    self.orig_id,
                    trip.origin
                );
            }
        }

        Ok(())
    }
}
