use std::collections::HashSet;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;

use map_model::{BuildingID, IntersectionID, Map, PathConstraints, PathRequest};
use sim::{IndividTrip, PersonSpec, TripEndpoint, TripMode, TripPurpose};

use crate::{Activity, CensusPerson, Config};

pub fn make_person(
    person: CensusPerson,
    map: &Map,
    rng: &mut XorShiftRng,
    config: &Config,
) -> PersonSpec {
    let schedule = person.generate_schedule(config, rng);

    let mut output = PersonSpec {
        orig_id: None,
        origin: TripEndpoint::Bldg(person.home),
        trips: Vec::new(),
    };

    // Only consider two-way intersections, so the agent can return the same way
    // they came.
    // TODO: instead, if it's not a two-way border, we should find an intersection
    // an incoming border "near" the outgoing border, to allow a broader set of
    // realistic options.
    // TODO: prefer larger thoroughfares to better reflect reality.
    let commuter_borders: Vec<IntersectionID> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|b| b.is_incoming_border())
        .map(|b| b.id)
        .collect();
    // TODO Where should we validate that at least one border exists? Probably in
    // generate_scenario, at minimum.

    let mut current_location = TripEndpoint::Bldg(person.home);
    for (departure_time, activity) in schedule.activities {
        // TODO This field isn't that important; later we could map Activity to a TripPurpose
        // better.
        // let purpose = TripPurpose::Shopping;

        // let goto = if let Some(destination) =
        //     find_building_for_activity(activity, current_location, map, rng)
        // {
        //     TripEndpoint::Bldg(destination)
        // } else {
        //     // No buildings satisfy the activity. Just go somewhere off-map.
        //     TripEndpoint::Border(*commuter_borders.choose(rng).unwrap())
        // };

        // let mode = pick_mode(current_location, goto, map, rng, config);
        // output
        //     .trips
        //     .push(IndividTrip::new(departure_time, purpose, goto, mode));

        // current_location = goto;
    }

    output
}

fn find_building_for_activity(
    activity: Activity,
    _start: TripEndpoint,
    map: &Map,
    _rng: &mut XorShiftRng,
) -> Option<BuildingID> {
    // What types of OpenStreetMap amenities will satisfy each activity?
    let categories: HashSet<&'static str> = match activity {
        Activity::Entertainment => vec!["cinema", "theatre"],
        // TODO Fill this out. amenity_type in map_gui/src/tools/mod.rs might be helpful. It might
        // also be helpful to edit the list of possible activities in lib.rs too.
        _ => vec![],
    }
    .into_iter()
    .collect();

    // Find all buildings with a matching amenity
    let mut candidates: Vec<BuildingID> = Vec::new();
    for b in map.all_buildings() {
        for amenity in &b.amenities {
            if categories.contains(amenity.amenity_type.as_str()) {
                candidates.push(b.id);
            }
        }
    }

    // TODO If there are several choices of building that satisfy an activity, which one will
    // someone choose? One simple approach could just calculate the difficulty of going from the
    // previous location (starting from home) to that place, using some mode of travel. Then either
    // pick the closest choice, or even better, randomize, but weight based on the cost of getting
    // there. map.pathfind() may be helpful.

    // For now, just pick the first choice arbitrarily
    candidates.get(0).cloned()
}

fn pick_mode(
    from: TripEndpoint,
    to: TripEndpoint,
    map: &Map,
    rng: &mut XorShiftRng,
    config: &Config,
) -> TripMode {
    let (b1, b2) = match (from, to) {
        (TripEndpoint::Bldg(b1), TripEndpoint::Bldg(b2)) => (b1, b2),
        // TODO Always drive when going on or off-map?
        _ => {
            return TripMode::Drive;
        }
    };

    // Decide mode based on walking distance
    let distance = if let Some(path) =
        PathRequest::between_buildings(map, b1, b2, PathConstraints::Pedestrian)
            .and_then(|req| map.pathfind(req))
    {
        path.total_length()
    } else {
        // If the buildings aren't connected, there was probably a bug importing the map. Just
        // fallback to driving. If the trip can't be started in the simulation, it'll show up as
        // cancelled with more details about the problem.
        return TripMode::Drive;
    };

    // TODO If either endpoint is in an access-restricted zone (like a living street), then
    // probably don't drive there. Actually, it depends on the specific tagging; access=no in the
    // US usually means a gated community.

    // TODO Make this probabilistic
    // for example probability of walking currently has massive differences
    // at thresholds, it would be nicer to change this gradually
    // TODO - do not select based on distance but select one that is fastest/best in the
    // given situation excellent bus connection / plenty of parking /
    // cycleways / suitable rail connection all strongly influence
    // selected mode of transport, distance is not the sole influence
    // in some cities there may case where driving is only possible method
    // to get somewhere, even at a short distance

    // Always walk for really short trips
    if distance < config.walk_for_distances_shorter_than {
        return TripMode::Walk;
    }

    // Sometimes bike or walk for moderate trips
    if distance < config.walk_or_bike_for_distances_shorter_than {
        // TODO We could move all of these params to Config, but I'm not sure if the overall flow
        // of logic in this functon is what we want yet.
        if rng.gen_bool(0.15) {
            return TripMode::Bike;
        }
        if rng.gen_bool(0.05) {
            return TripMode::Walk;
        }
    }

    // For longer trips, maybe bike for dedicated cyclists
    if rng.gen_bool(0.005) {
        return TripMode::Bike;
    }
    // Try transit if available, or fallback to walking
    if rng.gen_bool(0.3) {
        return TripMode::Transit;
    }

    // Most of the time, just drive
    TripMode::Drive
}