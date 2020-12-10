use std::collections::HashSet;

use rand_xorshift::XorShiftRng;

use map_model::{BuildingID, Map};
use sim::{IndividTrip, PersonSpec, TripEndpoint, TripMode, TripPurpose};

use crate::{Activity, CensusPerson, Config};

// sim/src/make/activity_model.rs does parts of this, but in a simplified way. It might be a good
// starting point though.

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

    let mut current_location = person.home;
    for (departure_time, activity) in schedule.activities {
        if let Some(destination) = find_building_for_activity(activity, current_location, map, rng)
        {
            let mode = pick_mode(current_location, destination, map, rng);
            output.trips.push(IndividTrip::new(
                departure_time,
                // TODO This field isn't that important; later we could map Activity to a
                // TripPurpose better.
                TripPurpose::Shopping,
                TripEndpoint::Bldg(destination),
                mode,
            ));
        } else {
            warn!(
                "No buildings satisfy activity {:?}. Skipping a step in the schedule",
                activity
            );
        }
    }

    // TODO For each activity, we have to pick a specific building to satisfy that activity.
    // map_gui/src/tools/mod.rs has amenity_type, which is an incomplete mapping from different
    // OpenStreetMap tags to types of businesses. Something like that could be helpful here.

    output
}

fn find_building_for_activity(
    activity: Activity,
    start: BuildingID,
    map: &Map,
    rng: &mut XorShiftRng,
) -> Option<BuildingID> {
    // What types of OpenStreetMap amenities will satisfy each activity?
    let categories: HashSet<&'static str> = match activity {
        Activity::Movies => vec!["cinema", "theatre"],
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

    // For no, just pick the first choice arbitrarily
    candidates.get(0).cloned()
}

// TODO
fn pick_mode(from: BuildingID, to: BuildingID, map: &Map, rng: &mut XorShiftRng) -> TripMode {
    TripMode::Drive
}
