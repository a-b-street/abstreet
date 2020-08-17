use crate::{
    IndividTrip, PersonID, PersonSpec, Scenario, ScenarioGenerator, SpawnTrip, TripEndpoint,
    TripMode,
};
use abstutil::{Parallelism, Timer};
use geom::{Distance, Duration, Time};
use map_model::{BuildingID, BuildingType, Intersection, Map, PathConstraints, PathRequest};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;

impl ScenarioGenerator {
    // Designed in https://github.com/dabreegster/abstreet/issues/154
    pub fn proletariat_robot(map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) -> Scenario {
        let mut residences: Vec<(BuildingID, XorShiftRng)> = Vec::new();
        let mut workplaces: Vec<BuildingID> = Vec::new();
        let mut num_residences = 0;
        for b in map.all_buildings() {
            match b.bldg_type {
                BuildingType::Residential(num_ppl) => {
                    for _ in 0..num_ppl {
                        residences.push((b.id, abstutil::fork_rng(rng)));
                    }
                    num_residences += 1;
                }
                BuildingType::ResidentialCommercial(num_ppl) => {
                    for _ in 0..num_ppl {
                        residences.push((b.id, abstutil::fork_rng(rng)));
                    }
                    workplaces.push(b.id);
                    num_residences += 1;
                }
                BuildingType::Commercial => {
                    workplaces.push(b.id);
                }
                BuildingType::Empty => {}
            }
        }

        let mut s = Scenario::empty(map, "random people going to/from work");
        // Include all buses/trains
        s.only_seed_buses = None;

        for mut person in timer
            .parallelize(
                "create people",
                Parallelism::Fastest,
                residences,
                |(home, mut forked_rng)| robot_person(home, &workplaces, map, &mut forked_rng),
            )
            .into_iter()
            .flatten()
        {
            person.id = PersonID(s.people.len());
            s.people.push(person);
        }

        // Create trips between map borders. For now, scale the number by the number of residences.
        let incoming_connections = map.all_incoming_borders();
        let outgoing_connections = map.all_outgoing_borders();
        for mut person in timer
            .parallelize(
                "create border trips",
                Parallelism::Fastest,
                std::iter::repeat_with(|| abstutil::fork_rng(rng))
                    .take(num_residences)
                    .collect(),
                |mut forked_rng| {
                    border_person(
                        &incoming_connections,
                        &outgoing_connections,
                        map,
                        &mut forked_rng,
                    )
                },
            )
            .into_iter()
            .flatten()
        {
            person.id = PersonID(s.people.len());
            s.people.push(person);
        }

        s
    }
}

// Make a person going from their home to a random workplace, then back again later.
fn robot_person(
    home: BuildingID,
    workplaces: &Vec<BuildingID>,
    map: &Map,
    rng: &mut XorShiftRng,
) -> Option<PersonSpec> {
    let work = *workplaces.choose(rng).unwrap();
    if home == work {
        // working and living in the same building
        return None;
    }
    // Decide mode based on walking distance. If the buildings aren't connected,
    // probably a bug in importing; just skip this person.
    let dist = map
        .pathfind(PathRequest {
            start: map.get_b(home).sidewalk_pos,
            end: map.get_b(work).sidewalk_pos,
            constraints: PathConstraints::Pedestrian,
        })?
        .total_length();
    // TODO If home or work is in an access-restricted zone (like a living street),
    // then probably don't drive there. Actually, it depends on the specific tagging;
    // access=no in the US usually means a gated community.
    let mode = select_trip_mode(dist, rng);

    // TODO This will cause a single morning and afternoon rush. Outside of these times,
    // it'll be really quiet. Probably want a normal distribution centered around these
    // peak times, but with a long tail.
    let mut depart_am = rand_time(
        rng,
        Time::START_OF_DAY + Duration::hours(7),
        Time::START_OF_DAY + Duration::hours(10),
    );
    let mut depart_pm = rand_time(
        rng,
        Time::START_OF_DAY + Duration::hours(17),
        Time::START_OF_DAY + Duration::hours(19),
    );

    if rng.gen_bool(0.1) {
        // hacky hack to get some background traffic
        depart_am = rand_time(
            rng,
            Time::START_OF_DAY + Duration::hours(0),
            Time::START_OF_DAY + Duration::hours(12),
        );
        depart_pm = rand_time(
            rng,
            Time::START_OF_DAY + Duration::hours(12),
            Time::START_OF_DAY + Duration::hours(24),
        );
    }

    // Skip the person if either trip can't be created.
    let goto_work = SpawnTrip::new(
        TripEndpoint::Bldg(home),
        TripEndpoint::Bldg(work),
        mode,
        map,
    )?;
    let return_home = SpawnTrip::new(
        TripEndpoint::Bldg(work),
        TripEndpoint::Bldg(home),
        mode,
        map,
    )?;

    Some(PersonSpec {
        // Fix this outside the parallelism
        id: PersonID(0),
        orig_id: None,
        trips: vec![
            IndividTrip::new(depart_am, goto_work),
            IndividTrip::new(depart_pm, return_home),
        ],
    })
}

fn border_person(
    incoming_connections: &Vec<&Intersection>,
    outgoing_connections: &Vec<&Intersection>,
    map: &Map,
    rng: &mut XorShiftRng,
) -> Option<PersonSpec> {
    // TODO it would be nice to weigh border points by for example lane count
    let random_incoming_border = incoming_connections.choose(rng).unwrap();
    let random_outgoing_border = outgoing_connections.choose(rng).unwrap();
    let b_random_incoming_border = incoming_connections.choose(rng).unwrap();
    let b_random_outgoing_border = outgoing_connections.choose(rng).unwrap();
    if random_incoming_border.id == random_outgoing_border.id
        || b_random_incoming_border.id == b_random_outgoing_border.id
    {
        return None;
    }
    // TODO calculate
    let distance_on_map = Distance::meters(2000.0);
    // TODO randomize
    // having random trip distance happening offscreen will allow things
    // like very short car trips, representing larger car trip happening mostly offscreen
    let distance_outside_map = Distance::meters(rng.gen_range(0.0, 20_000.0));
    let mode = select_trip_mode(distance_on_map + distance_outside_map, rng);
    let goto_work = SpawnTrip::new(
        TripEndpoint::Border(random_incoming_border.id, None),
        TripEndpoint::Border(random_outgoing_border.id, None),
        mode,
        map,
    )?;
    let return_home = SpawnTrip::new(
        TripEndpoint::Border(b_random_incoming_border.id, None),
        TripEndpoint::Border(b_random_outgoing_border.id, None),
        mode,
        map,
    )?;
    // TODO more reasonable time schedule, rush hour peak etc
    let depart_am = rand_time(
        rng,
        Time::START_OF_DAY + Duration::hours(0),
        Time::START_OF_DAY + Duration::hours(12),
    );
    let depart_pm = rand_time(
        rng,
        Time::START_OF_DAY + Duration::hours(12),
        Time::START_OF_DAY + Duration::hours(24),
    );
    Some(PersonSpec {
        id: PersonID(0),
        orig_id: None,
        trips: vec![
            IndividTrip::new(depart_am, goto_work),
            IndividTrip::new(depart_pm, return_home),
        ],
    })
}

fn select_trip_mode(distance: Distance, rng: &mut XorShiftRng) -> TripMode {
    // TODO Make this probabilistic
    // for example probability of walking currently has massive differences
    // at thresholds, it would be nicer to change this graduall
    // TODO - do not select based on distance but select one that is fastest/best in the
    // given situation excellent bus connection / plenty of parking /
    // cycleways / suitable rail connection all strongly influence
    // selected mode of transport, distance is not the sole influence
    // in some cities there may case where driving is only possible method
    // to get somewhere, even at a short distance
    if distance < Distance::miles(0.5) {
        return TripMode::Walk;
    }
    if rng.gen_bool(0.005) {
        // low chance for really, really dedicated cyclists
        return TripMode::Bike;
    }
    if rng.gen_bool(0.3) {
        // try transit if available, will
        // degrade into walk if not available
        return TripMode::Transit;
    }
    if distance < Distance::miles(3.0) {
        if rng.gen_bool(0.15) {
            return TripMode::Bike;
        }
        if rng.gen_bool(0.05) {
            return TripMode::Walk;
        }
    }
    TripMode::Drive
}

fn rand_time(rng: &mut XorShiftRng, low: Time, high: Time) -> Time {
    assert!(high > low);
    Time::START_OF_DAY + Duration::seconds(rng.gen_range(low.inner_seconds(), high.inner_seconds()))
}
