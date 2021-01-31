//! Some users of the API (https://a-b-street.github.io/docs/dev/api.html) have their own
//! simulation input data; import it here.

use anyhow::Result;
use serde::Deserialize;

use geom::{Distance, FindClosest, LonLat, Time};
use map_model::{IntersectionID, Map, PathConstraints};

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
    /// Import external scenario data. The main difference between `ExternalPerson` and
    /// `PersonSpec` is a way to specify endpoints by a `LonLat`. This is snapped to the nearest
    /// building. If the point is outside of the map boundary, it's snapped to the nearest border
    /// (by Euclidean distance -- the network outside the given map isn't known). Failure happens
    /// if a point is within the map, but not close enough to any buildings.
    pub fn import(map: &Map, input: Vec<ExternalPerson>) -> Result<Vec<PersonSpec>> {
        let mut closest: FindClosest<TripEndpoint> = FindClosest::new(map.get_bounds());
        for b in map.all_buildings() {
            closest.add(TripEndpoint::Bldg(b.id), b.polygon.points());
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
                            .min_by_key(|(_, border)| border.fast_dist(gps))
                            .ok_or_else(|| anyhow!("No border for {}", mode.ongoing_verb()))?
                            .0,
                    ))
                }
            }
        };

        let mut results = Vec::new();
        for person in input {
            let mut spec = PersonSpec {
                orig_id: None,
                origin: lookup_pt(person.origin, true, person.trips[0].mode)?,
                trips: Vec::new(),
            };
            for trip in person.trips {
                // TODO Add space in the API to specify purpose, but probably make it optional.
                spec.trips.push(IndividTrip::new(
                    trip.departure,
                    TripPurpose::Shopping,
                    // TODO Do we handle somebody going off-map via one one-way bridge, and
                    // re-entering using the other?
                    lookup_pt(trip.destination, false, trip.mode)?,
                    trip.mode,
                ));
            }
            results.push(spec);
        }
        Ok(results)
    }
}

pub struct MapBorders {
    pub incoming_walking: Vec<(IntersectionID, LonLat)>,
    pub incoming_driving: Vec<(IntersectionID, LonLat)>,
    pub incoming_biking: Vec<(IntersectionID, LonLat)>,
    pub outgoing_walking: Vec<(IntersectionID, LonLat)>,
    pub outgoing_driving: Vec<(IntersectionID, LonLat)>,
    pub outgoing_biking: Vec<(IntersectionID, LonLat)>,
}

impl MapBorders {
    pub fn new(map: &Map) -> MapBorders {
        let bounds = map.get_gps_bounds();
        let incoming_walking: Vec<(IntersectionID, LonLat)> = map
            .all_incoming_borders()
            .into_iter()
            .filter(|i| {
                !i.get_outgoing_lanes(map, PathConstraints::Pedestrian)
                    .is_empty()
            })
            .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
            .collect();
        let incoming_driving: Vec<(IntersectionID, LonLat)> = map
            .all_incoming_borders()
            .into_iter()
            .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Car).is_empty())
            .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
            .collect();
        let incoming_biking: Vec<(IntersectionID, LonLat)> = map
            .all_incoming_borders()
            .into_iter()
            .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Bike).is_empty())
            .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
            .collect();
        let outgoing_walking: Vec<(IntersectionID, LonLat)> = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|i| {
                !i.get_incoming_lanes(map, PathConstraints::Pedestrian)
                    .is_empty()
            })
            .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
            .collect();
        let outgoing_driving: Vec<(IntersectionID, LonLat)> = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Car).is_empty())
            .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
            .collect();
        let outgoing_biking: Vec<(IntersectionID, LonLat)> = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Bike).is_empty())
            .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
            .collect();
        MapBorders {
            incoming_walking,
            incoming_driving,
            incoming_biking,
            outgoing_walking,
            outgoing_driving,
            outgoing_biking,
        }
    }

    /// Returns the (incoming, outgoing) borders for the specififed mode.
    pub fn for_mode(
        &self,
        mode: TripMode,
    ) -> (
        &Vec<(IntersectionID, LonLat)>,
        &Vec<(IntersectionID, LonLat)>,
    ) {
        match mode {
            TripMode::Walk | TripMode::Transit => (&self.incoming_walking, &self.outgoing_walking),
            TripMode::Drive => (&self.incoming_driving, &self.outgoing_driving),
            TripMode::Bike => (&self.incoming_biking, &self.outgoing_biking),
        }
    }
}
