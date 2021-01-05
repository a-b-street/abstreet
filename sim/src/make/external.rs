//! Some users of the API (https://dabreegster.github.io/abstreet/dev/api.html) have their own
//! simulation input data; import it here.

use anyhow::Result;
use serde::Deserialize;

use geom::{Distance, FindClosest, LonLat, Time};
use map_model::Map;

use crate::{IndividTrip, PersonSpec, TripEndpoint, TripMode, TripPurpose};

#[derive(Deserialize)]
pub struct ExternalPerson {
    pub origin: ExternalTripEndpoint,
    pub trips: Vec<ExternalTrip>,
}

#[derive(Deserialize)]
pub struct ExternalTrip {
    pub departure: Time,
    pub destination: ExternalTripEndpoint,
    pub mode: TripMode,
}

#[derive(Deserialize)]
pub enum ExternalTripEndpoint {
    TripEndpoint(TripEndpoint),
    Position(LonLat),
}

impl ExternalPerson {
    pub fn import(map: &Map, input: Vec<ExternalPerson>) -> Result<Vec<PersonSpec>> {
        let mut closest: FindClosest<TripEndpoint> = FindClosest::new(map.get_bounds());
        for b in map.all_buildings() {
            closest.add(TripEndpoint::Bldg(b.id), b.polygon.points());
        }
        for i in map.all_intersections() {
            closest.add(TripEndpoint::Border(i.id), i.polygon.points());
        }
        let lookup_pt = |endpt| match endpt {
            ExternalTripEndpoint::TripEndpoint(endpt) => Ok(endpt),
            ExternalTripEndpoint::Position(gps) => {
                match closest.closest_pt(gps.to_pt(map.get_gps_bounds()), Distance::meters(100.0)) {
                    Some((x, _)) => Ok(x),
                    None => Err(anyhow!(
                        "No building or border intersection within 100m of {}",
                        gps
                    )),
                }
            }
        };

        let mut results = Vec::new();
        for person in input {
            let mut spec = PersonSpec {
                orig_id: None,
                origin: lookup_pt(person.origin)?,
                trips: Vec::new(),
            };
            for trip in person.trips {
                // TODO Add space in the API to specify purpose, but probably make it optional.
                spec.trips.push(IndividTrip::new(
                    trip.departure,
                    TripPurpose::Shopping,
                    lookup_pt(trip.destination)?,
                    trip.mode,
                ));
            }
            results.push(spec);
        }
        Ok(results)
    }
}
