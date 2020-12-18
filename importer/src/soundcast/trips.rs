use std::collections::HashMap;

use abstutil::{prettyprint_usize, MultiMap, Parallelism, Timer};
use geom::LonLat;
use map_model::{osm, BuildingID, IntersectionID, Map, PathConstraints, PathRequest, PathStep};
use sim::{IndividTrip, OrigPersonID, PersonSpec, Scenario, TripEndpoint, TripMode};

use crate::soundcast::popdat::{Endpoint, OrigTrip, PopDat};

#[derive(Clone, Debug)]
struct Trip {
    from: TripEndpoint,
    to: TripEndpoint,
    orig: OrigTrip,
}

// TODO Saying this function exploded in complexity is like saying I have coffee occasionally.
fn endpoints(
    from: &Endpoint,
    to: &Endpoint,
    map: &Map,
    osm_id_to_bldg: &HashMap<osm::OsmID, BuildingID>,
    (in_borders, out_borders): (
        &Vec<(IntersectionID, LonLat)>,
        &Vec<(IntersectionID, LonLat)>,
    ),
    constraints: PathConstraints,
    maybe_huge_map: Option<&(&Map, HashMap<osm::OsmID, BuildingID>)>,
) -> Option<(TripEndpoint, TripEndpoint)> {
    let from_bldg = from
        .osm_building
        .and_then(|id| osm_id_to_bldg.get(&id))
        .cloned();
    let to_bldg = to
        .osm_building
        .and_then(|id| osm_id_to_bldg.get(&id))
        .cloned();
    let border_endpt = match (from_bldg, to_bldg) {
        (Some(b1), Some(b2)) => {
            return Some((TripEndpoint::Bldg(b1), TripEndpoint::Bldg(b2)));
        }
        (Some(_), None) => to,
        (None, Some(_)) => from,
        (None, None) => {
            // TODO Detect and handle pass-through trips
            return None;
        }
    };
    let usable_borders = if from_bldg.is_some() {
        out_borders
    } else {
        in_borders
    };

    // The trip begins or ends at a border.
    // TODO It'd be nice to fix depart_at, trip_time, and trip_dist. Assume constant speed
    // through the trip. But when I last tried this, the distance was way off. :\

    let border_i = maybe_huge_map
        .and_then(|(huge_map, huge_osm_id_to_bldg)| {
            other_border(
                from,
                to,
                constraints,
                huge_map,
                huge_osm_id_to_bldg,
                map,
                usable_borders,
            )
        })
        .or_else(|| {
            // Fallback to finding the nearest border with straight-line distance
            usable_borders
                .iter()
                .min_by_key(|(_, pt)| pt.fast_dist(border_endpt.pos))
                .map(|(id, _)| *id)
        })?;
    let border = TripEndpoint::Border(border_i);
    if let Some(b) = from_bldg {
        Some((TripEndpoint::Bldg(b), border))
    } else {
        Some((border, TripEndpoint::Bldg(to_bldg.unwrap())))
    }
}

// Use the large map to find the real path somebody might take, then try to match that to a border
// in the smaller map.
fn other_border(
    from: &Endpoint,
    to: &Endpoint,
    constraints: PathConstraints,
    huge_map: &Map,
    huge_osm_id_to_bldg: &HashMap<osm::OsmID, BuildingID>,
    map: &Map,
    usable_borders: &Vec<(IntersectionID, LonLat)>,
) -> Option<IntersectionID> {
    let b1 = *from
        .osm_building
        .and_then(|id| huge_osm_id_to_bldg.get(&id))?;
    let b2 = *to
        .osm_building
        .and_then(|id| huge_osm_id_to_bldg.get(&id))?;
    let req = PathRequest::between_buildings(huge_map, b1, b2, constraints)?;
    let path = huge_map.pathfind(req).ok()?;

    // Do any of the usable borders match the path?
    // TODO Calculate this once
    let mut node_id_to_border = HashMap::new();
    for (i, _) in usable_borders {
        node_id_to_border.insert(map.get_i(*i).orig_id, *i);
    }
    for step in path.get_steps() {
        if let PathStep::Turn(t) = step {
            if let Some(i) = node_id_to_border.get(&huge_map.get_i(t.parent).orig_id) {
                return Some(*i);
            }
        }
    }
    None
}

fn clip_trips(map: &Map, popdat: &PopDat, huge_map: &Map, timer: &mut Timer) -> Vec<Trip> {
    let maybe_huge_map = if map.get_name().map == "huge_seattle" {
        None
    } else {
        let mut huge_osm_id_to_bldg = HashMap::new();
        for b in huge_map.all_buildings() {
            huge_osm_id_to_bldg.insert(b.orig_id, b.id);
        }
        Some((huge_map, huge_osm_id_to_bldg))
    };

    let mut osm_id_to_bldg = HashMap::new();
    for b in map.all_buildings() {
        osm_id_to_bldg.insert(b.orig_id, b.id);
    }
    let bounds = map.get_gps_bounds();
    let incoming_borders_walking: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| {
            !i.get_outgoing_lanes(map, PathConstraints::Pedestrian)
                .is_empty()
        })
        .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
        .collect();
    let incoming_borders_driving: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Car).is_empty())
        .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
        .collect();
    let incoming_borders_biking: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Bike).is_empty())
        .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
        .collect();
    let outgoing_borders_walking: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| {
            !i.get_incoming_lanes(map, PathConstraints::Pedestrian)
                .is_empty()
        })
        .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
        .collect();
    let outgoing_borders_driving: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Car).is_empty())
        .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
        .collect();
    let outgoing_borders_biking: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Bike).is_empty())
        .map(|i| (i.id, i.polygon.center().to_gps(bounds)))
        .collect();

    let total_trips = popdat.trips.len();
    let maybe_results: Vec<Option<Trip>> = timer.parallelize(
        "clip trips",
        Parallelism::Polite,
        popdat.trips.iter().collect(),
        |orig| {
            let (from, to) = endpoints(
                &orig.from,
                &orig.to,
                map,
                &osm_id_to_bldg,
                match orig.mode {
                    TripMode::Walk | TripMode::Transit => {
                        (&incoming_borders_walking, &outgoing_borders_walking)
                    }
                    TripMode::Drive => (&incoming_borders_driving, &outgoing_borders_driving),
                    TripMode::Bike => (&incoming_borders_biking, &outgoing_borders_biking),
                },
                match orig.mode {
                    TripMode::Walk | TripMode::Transit => PathConstraints::Pedestrian,
                    TripMode::Drive => PathConstraints::Car,
                    TripMode::Bike => PathConstraints::Bike,
                },
                maybe_huge_map.as_ref(),
            )?;
            Some(Trip {
                from,
                to,
                orig: orig.clone(),
            })
        },
    );
    let trips: Vec<Trip> = maybe_results.into_iter().flatten().collect();

    timer.note(format!(
        "{} trips clipped down to just {}",
        prettyprint_usize(total_trips),
        prettyprint_usize(trips.len())
    ));

    trips
}

pub fn make_weekday_scenario(
    map: &Map,
    popdat: &PopDat,
    huge_map: &Map,
    timer: &mut Timer,
) -> Scenario {
    // Include the origin with each trip
    let mut individ_trips: Vec<Option<(TripEndpoint, IndividTrip)>> = Vec::new();
    // person -> (trip seq, index into individ_trips)
    let mut trips_per_person: MultiMap<OrigPersonID, ((usize, bool, usize), usize)> =
        MultiMap::new();
    for trip in clip_trips(map, popdat, huge_map, timer) {
        let idx = individ_trips.len();
        individ_trips.push(Some((
            trip.from,
            IndividTrip::new(
                trip.orig.depart_at,
                trip.orig.purpose,
                trip.to,
                trip.orig.mode,
            ),
        )));
        trips_per_person.insert(trip.orig.person, (trip.orig.seq, idx));
    }
    timer.note(format!(
        "{} clipped trips, over {} people",
        prettyprint_usize(individ_trips.len()),
        prettyprint_usize(trips_per_person.len())
    ));

    let mut people = Vec::new();
    for (orig_id, seq_trips) in trips_per_person.consume() {
        let mut pairs = Vec::new();
        for (_, idx) in seq_trips {
            pairs.push(individ_trips[idx].take().unwrap());
        }
        // Actually, the sequence in the Soundcast dataset crosses midnight. Don't do that; sort by
        // departure time starting with midnight.
        pairs.sort_by_key(|(_, t)| t.depart);
        // Sanity check that endpoints match up
        for pair in pairs.windows(2) {
            let destination = &pair[0].1.destination;
            let origin = &pair[1].0;
            if destination != origin {
                timer.warn(format!(
                    "Skipping {:?}, with adjacent trips that warp from {:?} to {:?}",
                    orig_id, destination, origin
                ));
                continue;
            }
        }

        people.push(PersonSpec {
            orig_id: Some(orig_id),
            origin: pairs[0].0.clone(),
            trips: pairs.into_iter().map(|(_, t)| t).collect(),
        });
    }
    for maybe_t in individ_trips {
        if maybe_t.is_some() {
            panic!("Some IndividTrip wasn't associated with a Person?!");
        }
    }

    Scenario {
        scenario_name: "weekday".to_string(),
        map_name: map.get_name().clone(),
        people,
        only_seed_buses: None,
    }
    .remove_weird_schedules()
}
