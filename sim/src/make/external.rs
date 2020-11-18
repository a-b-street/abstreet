//! Some users of the API (https://dabreegster.github.io/abstreet/dev/api.html) have their own
//! simulation input data; import it here.

use serde::Deserialize;

use geom::{Distance, FindClosest, LonLat, Time};
use map_model::Map;

use crate::{IndividTrip, PersonID, PersonSpec, TripEndpoint, TripMode, TripPurpose};

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
    pub fn import(map: &Map, input: Vec<ExternalPerson>) -> Result<Vec<PersonSpec>, String> {
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
                    None => Err(format!(
                        "No building or border intersection within 100m of {}",
                        gps
                    )),
                }
            }
        };

        let mut results = Vec::new();
        for person in input {
            let mut spec = PersonSpec {
                id: PersonID(results.len()),
                orig_id: None,
                trips: Vec::new(),
            };
            let mut from = lookup_pt(person.origin)?;
            for trip in person.trips {
                let to = lookup_pt(trip.destination)?;
                // TODO Add space in the API to specify purpose, but probably make it optional.
                spec.trips.push(IndividTrip::new(
                    trip.departure,
                    TripPurpose::Shopping,
                    from.clone(),
                    to.clone(),
                    trip.mode,
                ));
                from = to;
            }
            results.push(spec);
        }
        Ok(results)
    }
}
