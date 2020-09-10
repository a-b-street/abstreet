use crate::{
    IndividTrip, PersonID, PersonSpec, Scenario, ScenarioGenerator, SpawnTrip, TripEndpoint,
    TripMode,
};
use abstutil::{prettyprint_usize, Parallelism, Timer};
use geom::{Distance, Duration, Time};
use map_model::{BuildingID, BuildingType, Map, PathConstraints, PathRequest};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;

impl ScenarioGenerator {
    // Designed in https://github.com/dabreegster/abstreet/issues/154
    pub fn proletariat_robot(map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) -> Scenario {
        let mut residents: Vec<BuildingID> = Vec::new();
        let mut workers: Vec<BuildingID> = Vec::new();

        let mut num_bldg_residential = 0;
        let mut num_bldg_commercial = 0;
        let mut num_bldg_mixed_residential_commercial = 0;
        for b in map.all_buildings() {
            match b.bldg_type {
                BuildingType::Residential(num_ppl) => {
                    if num_ppl == 0 {
                        trace!("empty Residential");
                    }

                    for _ in 0..num_ppl {
                        residents.push(b.id);
                    }
                    num_bldg_residential += 1;
                }
                BuildingType::ResidentialCommercial(num_ppl) => {
                    if num_ppl == 0 {
                        trace!("empty ResidentialCommercial");
                    }
                    for _ in 0..num_ppl {
                        residents.push(b.id);

                        // TODO: currently we just assume work capacity is the same as residential
                        // capacity, which is surely not true. How can we better estimate?
                        workers.push(b.id);
                    }
                    num_bldg_mixed_residential_commercial += 1;
                }
                BuildingType::Commercial => {
                    // TODO: currently we just assume some constant for jobs per building.
                    // A better metrics might be available parking or building size
                    let building_cap = usize::pow(2, rng.gen_range(0, 7));
                    for _ in 0..building_cap {
                        workers.push(b.id);
                    }
                    num_bldg_commercial += 1;
                }
                BuildingType::Empty => {}
            }
        }

        residents.shuffle(rng);
        workers.shuffle(rng);

        let mut s = Scenario::empty(map, "random people going to/from work");
        // Include all buses/trains
        s.only_seed_buses = None;

        let residents_cap = residents.len();
        let workers_cap = workers.len();

        // this saturation figure is an arbitrary guess - we assume that the number of trips will
        // scale as some factor of the people living and/or working on the map. A number of more
        // than 1.0 will primarily affect the number of "pass through" trips - people who neither
        // work nor live in the neighborhood.
        let trip_saturation = 1.2;
        let num_trips = (trip_saturation * (residents_cap + workers_cap) as f64) as usize;

        // bound probabilities to ensure we're getting some diveristy of agents
        let lower_bound_prob = 0.05;
        let upper_bound_prob = 0.90;
        let prob_local_resident = if workers_cap == 0 {
            lower_bound_prob
        } else {
            f64::min(
                upper_bound_prob,
                f64::max(lower_bound_prob, residents_cap as f64 / num_trips as f64),
            )
        };
        let prob_local_worker = f64::min(
            upper_bound_prob,
            f64::max(lower_bound_prob, workers_cap as f64 / num_trips as f64),
        );

        debug!(
            "BUILDINGS - workplaces: {}, residences: {}, mixed: {}",
            prettyprint_usize(num_bldg_commercial),
            prettyprint_usize(num_bldg_residential),
            prettyprint_usize(num_bldg_mixed_residential_commercial)
        );
        debug!(
            "CAPACITY - workers_cap: {}, residents_cap: {}, prob_local_worker: {:.1}%, \
             prob_local_resident: {:.1}%",
            prettyprint_usize(workers_cap),
            prettyprint_usize(residents_cap),
            prob_local_worker * 100.,
            prob_local_resident * 100.
        );

        let mut num_trips_local = 0;
        let mut num_trips_commuting_in = 0;
        let mut num_trips_commuting_out = 0;
        let mut num_trips_passthru = 0;
        timer.start("create people");

        // Only consider two-way intersections, so the agent can return the same way
        // they came.
        // TODO: instead, if it's not a two-way border, we should find an intersection
        // an incoming border "near" the outgoing border, to allow a broader set of
        // realistic options.
        // TODO: prefer larger thoroughfares to better reflect reality.
        let commuter_borders: Vec<TripEndpoint> = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|b| b.is_incoming_border())
            .map(|b| TripEndpoint::Border(b.id, None))
            .collect();
        assert!(commuter_borders.len() > 0);
        let person_params = (0..num_trips)
            .map(|_| {
                let (is_local_resident, is_local_worker) = (
                    rng.gen_bool(prob_local_resident),
                    rng.gen_bool(prob_local_worker),
                );
                let home = if is_local_resident {
                    if let Some(residence) = residents.pop() {
                        TripEndpoint::Bldg(residence)
                    } else {
                        commuter_borders.choose(rng).unwrap().clone()
                    }
                } else {
                    commuter_borders.choose(rng).unwrap().clone()
                };

                let work = if is_local_worker {
                    if let Some(workplace) = workers.pop() {
                        TripEndpoint::Bldg(workplace)
                    } else {
                        commuter_borders.choose(rng).unwrap().clone()
                    }
                } else {
                    commuter_borders.choose(rng).unwrap().clone()
                };

                match (&home, &work) {
                    (TripEndpoint::Bldg(_), TripEndpoint::Bldg(_)) => {
                        num_trips_local += 1;
                    }
                    (TripEndpoint::Bldg(_), TripEndpoint::Border(_, _)) => {
                        num_trips_commuting_out += 1;
                    }
                    (TripEndpoint::Border(_, _), TripEndpoint::Bldg(_)) => {
                        num_trips_commuting_in += 1;
                    }
                    (TripEndpoint::Border(_, _), TripEndpoint::Border(_, _)) => {
                        num_trips_passthru += 1;
                    }
                };

                (home, work, abstutil::fork_rng(rng))
            })
            .collect();

        timer
            .parallelize(
                "create people: making PersonSpec from endpoints",
                Parallelism::Fastest,
                person_params,
                |(home, work, mut rng)| match create_prole(&home, &work, map, &mut rng) {
                    Ok(person) => Some(person),
                    Err(e) => {
                        trace!("Unable to create person. error: {}", e);
                        None
                    }
                },
            )
            .into_iter()
            .flatten()
            .for_each(|mut person| {
                person.id = PersonID(s.people.len());
                s.people.push(person);
            });

        timer.stop("create people");

        info!(
            "TRIPS - total: {}, local: {}, commuting_in: {}, commuting_out: {}, passthru: {}, \
             errored: {}, leftover_resident_capacity: {}, leftover_worker_capacity: {}",
            prettyprint_usize(num_trips),
            prettyprint_usize(num_trips_local),
            prettyprint_usize(num_trips_commuting_in),
            prettyprint_usize(num_trips_commuting_out),
            prettyprint_usize(num_trips_passthru),
            prettyprint_usize(num_trips - s.people.len()),
            prettyprint_usize(residents.len()),
            prettyprint_usize(workers.len()),
        );
        s
    }
}

fn create_prole(
    home: &TripEndpoint,
    work: &TripEndpoint,
    map: &Map,
    rng: &mut XorShiftRng,
) -> Result<PersonSpec, Box<dyn std::error::Error>> {
    if home == work {
        // TODO: handle edge-case of working and living in the same building...  maybe more likely
        // to go for a walk later in the day or something
        return Err("TODO: handle working and living in the same building".into());
    }

    let mode = match (home, work) {
        // commuting entirely within map
        (TripEndpoint::Bldg(home_bldg), TripEndpoint::Bldg(work_bldg)) => {
            // Decide mode based on walking distance. If the buildings aren't connected,
            // probably a bug in importing; just skip this person.
            let dist = if let Some(dist) = map
                .pathfind(PathRequest {
                    start: map.get_b(*home_bldg).sidewalk_pos,
                    end: map.get_b(*work_bldg).sidewalk_pos,
                    constraints: PathConstraints::Pedestrian,
                })
                .map(|p| p.total_length())
            {
                dist
            } else {
                return Err("no path found".into());
            };

            // TODO If home or work is in an access-restricted zone (like a living street),
            // then probably don't drive there. Actually, it depends on the specific tagging;
            // access=no in the US usually means a gated community.
            select_trip_mode(dist, rng)
        }
        // if you exit or leave the map, we assume driving
        _ => TripMode::Drive,
    };

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

    let goto_work = SpawnTrip::new(home.clone(), work.clone(), mode, map)
        .ok_or("unable to spawn 'goto work' trip")?;
    let return_home = SpawnTrip::new(work.clone(), home.clone(), mode, map)
        .ok_or("unable to spawn 'return home' trip")?;

    Ok(PersonSpec {
        // Fix this outside the parallelism
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
    // at thresholds, it would be nicer to change this gradually
    // TODO - do not select based on distance but select one that is fastest/best in the
    // given situation excellent bus connection / plenty of parking /
    // cycleways / suitable rail connection all strongly influence
    // selected mode of transport, distance is not the sole influence
    // in some cities there may case where driving is only possible method
    // to get somewhere, even at a short distance

    // Always walk for really short trips
    if distance < Distance::miles(0.5) {
        return TripMode::Walk;
    }

    // Sometimes bike or walk for moderate trips
    if distance < Distance::miles(3.0) {
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

fn rand_time(rng: &mut XorShiftRng, low: Time, high: Time) -> Time {
    assert!(high > low);
    Time::START_OF_DAY + Duration::seconds(rng.gen_range(low.inner_seconds(), high.inner_seconds()))
}
