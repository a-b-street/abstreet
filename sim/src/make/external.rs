// Some users of the API (https://dabreegster.github.io/abstreet/dev/api.html) have their own
// simulation input data; import it here.

use serde::Deserialize;

use geom::{Distance, FindClosest, LonLat, Pt2D, Time};
use map_model::Map;

use crate::{IndividTrip, PersonID, PersonSpec, SpawnTrip, TripEndpoint, TripMode, TripPurpose};

#[derive(Deserialize)]
pub struct ExternalPerson {
    pub origin: LonLat,
    pub trips: Vec<ExternalTrip>,
}

#[derive(Deserialize)]
pub struct ExternalTrip {
    pub departure: Time,
    pub position: LonLat,
    pub mode: TripMode,
}

impl ExternalPerson {
    pub fn import(map: &Map, input: Vec<ExternalPerson>) -> Result<Vec<PersonSpec>, String> {
        let mut closest: FindClosest<TripEndpoint> = FindClosest::new(map.get_bounds());
        for b in map.all_buildings() {
            closest.add(TripEndpoint::Bldg(b.id), b.polygon.points());
        }
        for i in map.all_intersections() {
            closest.add(TripEndpoint::Border(i.id, None), i.polygon.points());
        }
        let lookup_pt = |gps| match closest.closest_pt(
            Pt2D::from_gps(gps, map.get_gps_bounds()),
            Distance::meters(100.0),
        ) {
            Some((x, _)) => Ok(x),
            None => Err(format!(
                "No building or border intersection within 100m of {}",
                gps
            )),
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
                let to = lookup_pt(trip.position)?;
                if let Some(t) = SpawnTrip::new(from.clone(), to.clone(), trip.mode, &map) {
                    // TODO Add space in the API to specify purpose, but probably make it optional.
                    spec.trips
                        .push(IndividTrip::new(trip.departure, TripPurpose::Shopping, t));
                    from = to;
                } else {
                    return Err(format!(
                        "Can't make a {} trip from {:?} to {:?}",
                        trip.mode.ongoing_verb(),
                        from,
                        to
                    ));
                }
            }
            results.push(spec);
        }
        Ok(results)
    }
}
