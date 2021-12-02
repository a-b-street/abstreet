use std::collections::HashMap;

use abstutil::{prettyprint_usize, MultiMap, Timer};
use geom::{LonLat, PolyLine};
use map_model::{
    osm, BuildingID, IntersectionID, Map, Path, PathConstraints, PathRequest, PathStep,
};
use sim::{IndividTrip, MapBorders, OrigPersonID, PersonSpec, Scenario, TripEndpoint, TripMode};

use crate::soundcast::popdat::{Endpoint, OrigTrip, PopDat};

#[derive(Clone, Debug)]
struct Trip {
    from: TripEndpoint,
    to: TripEndpoint,
    orig: OrigTrip,
}

/// Transform the Seattle-wide `Endpoints` into specific `TripEndpoints` for this map. When the
/// endpoint happens to be a building on the map, this is straightforward. Otherwise, the endpoint
/// will snap to a border intersection.
///
/// When `only_passthrough_trips` is true, only trips beginning and ending off-map are returned.
/// When it's false, all other trips are returned.
#[allow(clippy::too_many_arguments)]
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
    only_passthrough_trips: bool,
) -> Option<(TripEndpoint, TripEndpoint)> {
    let from_bldg = from
        .osm_building
        .and_then(|id| osm_id_to_bldg.get(&id))
        .map(|b| TripEndpoint::Bldg(*b));
    let to_bldg = to
        .osm_building
        .and_then(|id| osm_id_to_bldg.get(&id))
        .map(|b| TripEndpoint::Bldg(*b));

    if only_passthrough_trips {
        if from_bldg.is_some() || to_bldg.is_some() {
            return None;
        }
    } else {
        // Easy case: totally within the map
        if let (Some(b1), Some(b2)) = (from_bldg, to_bldg) {
            return Some((b1, b2));
        }
    }

    // If it's a pass-through trip, check if the straight line between the endpoints even crosses
    // the map boundary. If not, the trip probably doesn't even involve this map at all. There are
    // false positives and negatives with this approach; we could be more accurate by pathfinding
    // on the huge_map, but that would be incredibly slow.
    if from_bldg.is_none() && to_bldg.is_none() {
        // TODO Don't enable pass-through trips yet in general. The time to generate the scenario, the
        // resulting scenario file, and the simulation runtime (and gridlockiness) all skyrocket.
        // Need to harden more things before enabling.
        if !only_passthrough_trips {
            return None;
        }

        if let Ok(pl) = PolyLine::new(vec![
            from.pos.to_pt(map.get_gps_bounds()),
            to.pos.to_pt(map.get_gps_bounds()),
        ]) {
            if !map.get_boundary_polygon().intersects_polyline(&pl) {
                return None;
            }
        }
    }

    // TODO When the trip begins or ends at a border, it'd be nice to fix depart_at, trip_time, and
    // trip_dist. Assume constant speed through the trip. But when I last tried this, the distance
    // was way off. :\

    let snapper = BorderSnapper::new(from, to, constraints, maybe_huge_map)
        .unwrap_or(BorderSnapper { path: None });

    let from_endpt = from_bldg
        .or_else(|| snapper.snap_border(in_borders, true, map, maybe_huge_map))
        .or_else(|| {
            // Fallback to finding the nearest border with straight-line distance
            in_borders
                .iter()
                .min_by_key(|(_, pt)| pt.fast_dist(from.pos))
                .map(|(id, _)| TripEndpoint::Border(*id))
        })?;
    let to_endpt = to_bldg
        .or_else(|| snapper.snap_border(out_borders, false, map, maybe_huge_map))
        .or_else(|| {
            // Fallback to finding the nearest border with straight-line distance
            out_borders
                .iter()
                .min_by_key(|(_, pt)| pt.fast_dist(to.pos))
                .map(|(id, _)| TripEndpoint::Border(*id))
        })?;

    if from_endpt == to_endpt {
        //warn!("loop on {:?}. {:?} to {:?}", from_endpt, from, to);
    }

    Some((from_endpt, to_endpt))
}

// Use the large map to find the real path somebody might take, then try to match that to a border
// in the smaller map.
struct BorderSnapper {
    path: Option<Path>,
}

impl BorderSnapper {
    fn new(
        from: &Endpoint,
        to: &Endpoint,
        constraints: PathConstraints,
        maybe_huge_map: Option<&(&Map, HashMap<osm::OsmID, BuildingID>)>,
    ) -> Option<BorderSnapper> {
        let (huge_map, huge_osm_id_to_bldg) = maybe_huge_map?;
        let b1 = *from
            .osm_building
            .and_then(|id| huge_osm_id_to_bldg.get(&id))?;
        let b2 = *to
            .osm_building
            .and_then(|id| huge_osm_id_to_bldg.get(&id))?;
        let req = PathRequest::between_buildings(huge_map, b1, b2, constraints)?;
        Some(BorderSnapper {
            path: huge_map.pathfind(req).ok(),
        })
    }

    fn snap_border(
        &self,
        usable_borders: &[(IntersectionID, LonLat)],
        incoming: bool,
        map: &Map,
        maybe_huge_map: Option<&(&Map, HashMap<osm::OsmID, BuildingID>)>,
    ) -> Option<TripEndpoint> {
        let huge_map = maybe_huge_map?.0;
        // Do any of the usable borders match the path?
        // TODO Calculate this once
        let mut node_id_to_border = HashMap::new();
        for (i, _) in usable_borders {
            node_id_to_border.insert(map.get_i(*i).orig_id, *i);
        }
        let mut iter1;
        let mut iter2;
        let steps: &mut dyn Iterator<Item = &PathStep> = if incoming {
            iter1 = self.path.as_ref()?.get_steps().iter();
            &mut iter1
        } else {
            iter2 = self.path.as_ref()?.get_steps().iter().rev();
            &mut iter2
        };
        for step in steps {
            if let PathStep::Turn(t) | PathStep::ContraflowTurn(t) = step {
                if let Some(i) = node_id_to_border.get(&huge_map.get_i(t.parent).orig_id) {
                    return Some(TripEndpoint::Border(*i));
                }
            }
        }
        None
    }
}

fn clip_trips(
    map: &Map,
    popdat: &PopDat,
    huge_map: &Map,
    only_passthrough_trips: bool,
    timer: &mut Timer,
) -> Vec<Trip> {
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
    let borders = MapBorders::new(map);

    let total_trips = popdat.trips.len();
    let maybe_results: Vec<Option<Trip>> =
        timer.parallelize("clip trips", popdat.trips.iter().collect(), |orig| {
            let (from, to) = endpoints(
                &orig.from,
                &orig.to,
                map,
                &osm_id_to_bldg,
                borders.for_mode(orig.mode),
                match orig.mode {
                    TripMode::Walk | TripMode::Transit => PathConstraints::Pedestrian,
                    TripMode::Drive => PathConstraints::Car,
                    TripMode::Bike => PathConstraints::Bike,
                },
                maybe_huge_map.as_ref(),
                only_passthrough_trips,
            )?;
            Some(Trip {
                from,
                to,
                orig: orig.clone(),
            })
        });
    let trips: Vec<Trip> = maybe_results.into_iter().flatten().collect();

    info!(
        "{} trips clipped down to just {}",
        prettyprint_usize(total_trips),
        prettyprint_usize(trips.len())
    );

    trips
}

pub fn make_scenario(
    scenario_name: &str,
    map: &Map,
    popdat: &PopDat,
    huge_map: &Map,
    timer: &mut Timer,
) -> Scenario {
    let only_passthrough_trips = scenario_name == "passthrough";

    let mut individ_trips: Vec<Option<IndividTrip>> = Vec::new();
    // person -> (trip seq, index into individ_trips)
    let mut trips_per_person: MultiMap<OrigPersonID, ((usize, bool, usize), usize)> =
        MultiMap::new();
    for trip in clip_trips(map, popdat, huge_map, only_passthrough_trips, timer) {
        let idx = individ_trips.len();
        individ_trips.push(Some(IndividTrip::new(
            trip.orig.depart_at,
            trip.orig.purpose,
            trip.from,
            trip.to,
            trip.orig.mode,
        )));
        trips_per_person.insert(trip.orig.person, (trip.orig.seq, idx));
    }
    info!(
        "{} clipped trips, over {} people",
        prettyprint_usize(individ_trips.len()),
        prettyprint_usize(trips_per_person.len())
    );

    let mut people = Vec::new();
    for (orig_id, seq_trips) in trips_per_person.consume() {
        let mut trips = Vec::new();
        for (_, idx) in seq_trips {
            trips.push(individ_trips[idx].take().unwrap());
        }
        // Actually, the sequence in the Soundcast dataset crosses midnight. Don't do that; sort by
        // departure time starting with midnight.
        trips.sort_by_key(|t| t.depart);
        // Sanity check that endpoints match up
        for pair in trips.windows(2) {
            let destination = &pair[0].destination;
            let origin = &pair[1].origin;
            if destination != origin {
                warn!(
                    "Skipping {:?}, with adjacent trips that warp from {:?} to {:?}",
                    orig_id, destination, origin
                );
                continue;
            }
        }

        people.push(PersonSpec {
            orig_id: Some(orig_id),
            trips,
        });
    }
    for maybe_t in individ_trips {
        if maybe_t.is_some() {
            panic!("Some IndividTrip wasn't associated with a Person?!");
        }
    }

    Scenario {
        scenario_name: scenario_name.to_string(),
        map_name: map.get_name().clone(),
        people,
        only_seed_buses: None,
    }
    .remove_weird_schedules()
}
