//! Some users of the API (https://a-b-street.github.io/docs/tech/dev/api.html) have their own
//! simulation input data; import it here.

use anyhow::Result;
use serde::Deserialize;

use geom::{Distance, FindClosest, LonLat, Time};
use map_model::Map;

use crate::{IndividTrip, MapBorders, PersonSpec, TripEndpoint, TripMode, TripPurpose};

#[derive(Deserialize)]
pub struct ExternalPerson {
    pub trips: Vec<ExternalTrip>,
}

#[derive(Deserialize)]
pub struct ExternalTrip {
    pub departure: Time,
    pub origin: ExternalTripEndpoint,
    pub destination: ExternalTripEndpoint,
    pub mode: TripMode,
    pub purpose: TripPurpose,
}

#[derive(Deserialize)]
pub enum ExternalTripEndpoint {
    TripEndpoint(TripEndpoint),
    Position(LonLat),
}

impl ExternalPerson {
    /// Import external scenario data. The main difference between `ExternalPerson` and
    /// `PersonSpec` is a way to specify endpoints by a `LonLat`. This is snapped to the nearest
    /// building. If the point is outside of the map boundary, it's snapped to the nearest border
    /// (by Euclidean distance -- the network outside the given map isn't known). Failure happens
    /// if a point is within the map, but not close enough to any buildings. If `skip_problems` is
    /// true, then those failures are logged; otherwise this panics at the first problem.
    pub fn import(
        map: &Map,
        input: Vec<ExternalPerson>,
        skip_problems: bool,
    ) -> Result<Vec<PersonSpec>> {
        let mut closest: FindClosest<TripEndpoint> = FindClosest::new();
        for b in map.all_buildings() {
            closest.add_polygon(TripEndpoint::Building(b.id), &b.polygon);
        }
        let borders = MapBorders::new(map);

        let lookup_pt = |endpt, is_origin, mode| match endpt {
            ExternalTripEndpoint::TripEndpoint(endpt) => Ok(endpt),
            ExternalTripEndpoint::Position(gps) => {
                let pt = gps.to_pt(map.get_gps_bounds());
                if map.get_boundary_polygon().contains_pt(pt) {
                    match closest.closest_pt(pt, Distance::meters(100.0)) {
                        Some((x, _)) => Ok(x),
                        None => Err(anyhow!("No building within 100m of {}", gps)),
                    }
                } else {
                    let (incoming, outgoing) = borders.for_mode(mode);
                    let candidates = if is_origin { incoming } else { outgoing };
                    Ok(TripEndpoint::Border(
                        candidates
                            .iter()
                            .min_by_key(|border| border.gps_pos.fast_dist(gps))
                            .ok_or_else(|| anyhow!("No border for {}", mode.ongoing_verb()))?
                            .i,
                    ))
                }
            }
        };

        let mut results = Vec::new();
        for person in input {
            let mut spec = PersonSpec {
                orig_id: None,
                trips: Vec::new(),
            };
            for trip in person.trips {
                if trip.departure < Time::START_OF_DAY {
                    if skip_problems {
                        warn!(
                            "Skipping trip with negative departure time {:?}",
                            trip.departure
                        );
                        continue;
                    } else {
                        bail!("Some trip has negative departure time {:?}", trip.departure);
                    }
                }

                spec.trips.push(IndividTrip::new(
                    trip.departure,
                    trip.purpose,
                    match lookup_pt(trip.origin, true, trip.mode) {
                        Ok(endpt) => endpt,
                        Err(err) => {
                            if skip_problems {
                                warn!("Skipping person: {}", err);
                                continue;
                            } else {
                                return Err(err);
                            }
                        }
                    },
                    match lookup_pt(trip.destination, false, trip.mode) {
                        Ok(endpt) => endpt,
                        Err(err) => {
                            if skip_problems {
                                warn!("Skipping person: {}", err);
                                continue;
                            } else {
                                return Err(err);
                            }
                        }
                    },
                    trip.mode,
                ));
            }
            results.push(spec);
        }
        Ok(results)
    }
}
