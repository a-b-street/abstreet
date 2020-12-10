use rand_xorshift::XorShiftRng;

use map_model::Map;
use sim::PersonSpec;

use crate::{CensusPerson, Config};

// sim/src/make/activity_model.rs does parts of this, but in a simplified way. It might be a good
// starting point though.

pub fn make_person(
    person: CensusPerson,
    map: &Map,
    rng: &mut XorShiftRng,
    config: &Config,
) -> PersonSpec {
    let schedule = person.generate_schedule(config, rng);

    // TODO For each activity, we have to pick a specific building to satisfy that activity.
    // map_gui/src/tools/mod.rs has amenity_type, which is an incomplete mapping from different
    // OpenStreetMap tags to types of businesses. Something like that could be helpful here.

    // TODO If there are several choices of building that satisfy an activity, which one will
    // someone choose? One simple approach could just calculate the difficulty of going from the
    // previous location (starting from home) to that place, using some mode of travel. Then either
    // pick the closest choice, or even better, randomize, but weight based on the cost of getting
    // there. map.pathfind() may be helpful.

    // TODO For each trip between two buildings, what mode will the person pick? (TripMode::{Drive,
    // Walk, Bike, Transit})

    todo!()
}
