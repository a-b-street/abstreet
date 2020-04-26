use crate::soundcast::popdat::{Endpoint, OrigTrip, PopDat};
use abstutil::{prettyprint_usize, MultiMap, Timer};
use geom::LonLat;
use map_model::{BuildingID, IntersectionID, Map, PathConstraints};
use sim::{
    DrivingGoal, IndividTrip, OffMapLocation, PersonID, PersonSpec, Scenario, SidewalkSpot,
    SpawnTrip, TripMode,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct Trip {
    from: TripEndpt,
    to: TripEndpt,
    orig: OrigTrip,
}

#[derive(Clone, Debug)]
enum TripEndpt {
    Building(BuildingID),
    Border(IntersectionID, OffMapLocation),
}

impl Trip {
    fn to_spawn_trip(&self, map: &Map) -> SpawnTrip {
        match self.orig.mode {
            TripMode::Drive => match self.from {
                TripEndpt::Border(i, ref origin) => SpawnTrip::FromBorder {
                    dr: map.get_i(i).some_outgoing_road(map),
                    goal: self.to.driving_goal(PathConstraints::Car, map),
                    is_bike: false,
                    origin: Some(origin.clone()),
                },
                TripEndpt::Building(b) => {
                    SpawnTrip::UsingParkedCar(b, self.to.driving_goal(PathConstraints::Car, map))
                }
            },
            TripMode::Bike => match self.from {
                TripEndpt::Building(b) => SpawnTrip::UsingBike(
                    SidewalkSpot::building(b, map),
                    self.to.driving_goal(PathConstraints::Bike, map),
                ),
                TripEndpt::Border(i, ref origin) => SpawnTrip::FromBorder {
                    dr: map.get_i(i).some_outgoing_road(map),
                    goal: self.to.driving_goal(PathConstraints::Bike, map),
                    is_bike: true,
                    origin: Some(origin.clone()),
                },
            },
            TripMode::Walk => SpawnTrip::JustWalking(
                self.from.start_sidewalk_spot(map),
                self.to.end_sidewalk_spot(map),
            ),
            TripMode::Transit => {
                let start = self.from.start_sidewalk_spot(map);
                let goal = self.to.end_sidewalk_spot(map);
                if let Some((stop1, stop2, route)) =
                    map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
                {
                    SpawnTrip::UsingTransit(start, goal, route, stop1, stop2)
                } else {
                    //timer.warn(format!("{:?} not actually using transit, because pathfinding
                    // didn't find any useful route", trip));
                    SpawnTrip::JustWalking(start, goal)
                }
            }
        }
    }
}

impl TripEndpt {
    fn new(
        endpt: &Endpoint,
        osm_id_to_bldg: &HashMap<i64, BuildingID>,
        borders: &Vec<(IntersectionID, LonLat)>,
    ) -> Option<TripEndpt> {
        if let Some(b) = endpt.osm_building.and_then(|id| osm_id_to_bldg.get(&id)) {
            return Some(TripEndpt::Building(*b));
        }
        borders
            .iter()
            .min_by_key(|(_, pt)| pt.fast_dist(endpt.pos))
            .map(|(id, _)| {
                TripEndpt::Border(
                    *id,
                    OffMapLocation {
                        gps: endpt.pos,
                        parcel_id: endpt.parcel_id,
                    },
                )
            })
    }

    fn start_sidewalk_spot(&self, map: &Map) -> SidewalkSpot {
        match self {
            TripEndpt::Building(b) => SidewalkSpot::building(*b, map),
            TripEndpt::Border(i, origin) => {
                SidewalkSpot::start_at_border(*i, Some(origin.clone()), map).unwrap()
            }
        }
    }

    fn end_sidewalk_spot(&self, map: &Map) -> SidewalkSpot {
        match self {
            TripEndpt::Building(b) => SidewalkSpot::building(*b, map),
            TripEndpt::Border(i, destination) => {
                SidewalkSpot::end_at_border(*i, Some(destination.clone()), map).unwrap()
            }
        }
    }

    fn driving_goal(&self, constraints: PathConstraints, map: &Map) -> DrivingGoal {
        match self {
            TripEndpt::Building(b) => DrivingGoal::ParkNear(*b),
            TripEndpt::Border(i, destination) => DrivingGoal::end_at_border(
                map.get_i(*i).some_incoming_road(map),
                constraints,
                Some(destination.clone()),
                map,
            )
            .unwrap(),
        }
    }
}

fn clip_trips(map: &Map, timer: &mut Timer) -> Vec<Trip> {
    let popdat: PopDat = abstutil::read_binary(abstutil::path_popdat(), timer);

    let mut osm_id_to_bldg = HashMap::new();
    for b in map.all_buildings() {
        osm_id_to_bldg.insert(b.osm_way_id, b.id);
    }
    let bounds = map.get_gps_bounds();
    // TODO Figure out why some polygon centers are broken
    let incoming_borders_walking: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| {
            !i.get_outgoing_lanes(map, PathConstraints::Pedestrian)
                .is_empty()
        })
        .filter_map(|i| i.polygon.center().to_gps(bounds).map(|pt| (i.id, pt)))
        .collect();
    let incoming_borders_driving: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Car).is_empty())
        .filter_map(|i| i.polygon.center().to_gps(bounds).map(|pt| (i.id, pt)))
        .collect();
    let incoming_borders_biking: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Bike).is_empty())
        .filter_map(|i| i.polygon.center().to_gps(bounds).map(|pt| (i.id, pt)))
        .collect();
    let outgoing_borders_walking: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| {
            !i.get_incoming_lanes(map, PathConstraints::Pedestrian)
                .is_empty()
        })
        .filter_map(|i| i.polygon.center().to_gps(bounds).map(|pt| (i.id, pt)))
        .collect();
    let outgoing_borders_driving: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Car).is_empty())
        .filter_map(|i| i.polygon.center().to_gps(bounds).map(|pt| (i.id, pt)))
        .collect();
    let outgoing_borders_biking: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Bike).is_empty())
        .filter_map(|i| i.polygon.center().to_gps(bounds).map(|pt| (i.id, pt)))
        .collect();

    let total_trips = popdat.trips.len();
    let maybe_results: Vec<Option<Trip>> = timer.parallelize("clip trips", popdat.trips, |orig| {
        let from = TripEndpt::new(
            &orig.from,
            &osm_id_to_bldg,
            match orig.mode {
                TripMode::Walk | TripMode::Transit => &incoming_borders_walking,
                TripMode::Drive => &incoming_borders_driving,
                TripMode::Bike => &incoming_borders_biking,
            },
        )?;
        let to = TripEndpt::new(
            &orig.to,
            &osm_id_to_bldg,
            match orig.mode {
                TripMode::Walk | TripMode::Transit => &outgoing_borders_walking,
                TripMode::Drive => &outgoing_borders_driving,
                TripMode::Bike => &outgoing_borders_biking,
            },
        )?;

        let trip = Trip { from, to, orig };

        match (&trip.from, &trip.to) {
            (TripEndpt::Border(_, _), TripEndpt::Border(_, _)) => {
                // TODO Detect and handle pass-through trips
                return None;
            }
            // Fix depart_at, trip_time, and trip_dist for border cases. Assume constant speed
            // through the trip.
            // TODO Disabled because slow and nonsensical distance ratios. :(
            (TripEndpt::Border(_, _), TripEndpt::Building(_)) => {}
            (TripEndpt::Building(_), TripEndpt::Border(_, _)) => {}
            (TripEndpt::Building(_), TripEndpt::Building(_)) => {}
        }

        Some(trip)
    });
    let trips: Vec<Trip> = maybe_results.into_iter().flatten().collect();

    timer.note(format!(
        "{} trips clipped down to just {}",
        prettyprint_usize(total_trips),
        prettyprint_usize(trips.len())
    ));

    trips
}

pub fn make_weekday_scenario(map: &Map, timer: &mut Timer) -> Scenario {
    let trips = clip_trips(map, timer);
    let orig_trips = trips.len();

    let mut individ_trips: Vec<Option<IndividTrip>> = Vec::new();
    // person -> (trip seq, index into individ_trips)
    let mut trips_per_person: MultiMap<(usize, usize), ((usize, bool, usize), usize)> =
        MultiMap::new();
    for (trip, depart, person, seq) in
        timer.parallelize("turn Soundcast trips into SpawnTrips", trips, |trip| {
            (
                trip.to_spawn_trip(map),
                trip.orig.depart_at,
                trip.orig.person,
                trip.orig.seq,
            )
        })
    {
        let idx = individ_trips.len();
        individ_trips.push(Some(IndividTrip { depart, trip }));
        trips_per_person.insert(person, (seq, idx));
    }
    timer.note(format!(
        "{} clipped trips down to {}, over {} people",
        prettyprint_usize(orig_trips),
        prettyprint_usize(individ_trips.len()),
        prettyprint_usize(trips_per_person.len())
    ));

    let mut people = Vec::new();
    for (orig_id, seq_trips) in trips_per_person.consume() {
        let id = PersonID(people.len());
        let mut trips = Vec::new();
        for (_, idx) in seq_trips {
            trips.push(individ_trips[idx].take().unwrap());
        }
        // Actually, the sequence in the Soundcast dataset crosses midnight. Don't do that; sort by
        // departure time starting with midnight.
        trips.sort_by_key(|t| t.depart);

        people.push(PersonSpec { id, orig_id, trips });
    }
    for maybe_t in individ_trips {
        if maybe_t.is_some() {
            panic!("Some IndividTrip wasn't associated with a Person?!");
        }
    }

    Scenario {
        scenario_name: "weekday".to_string(),
        map_name: map.get_name().to_string(),
        people,
        only_seed_buses: None,
    }
    .remove_weird_schedules(map)
}

pub fn make_weekday_scenario_with_everyone(map: &Map, timer: &mut Timer) -> Scenario {
    let popdat: PopDat = abstutil::read_binary(abstutil::path_popdat(), timer);

    let mut individ_trips: Vec<Option<IndividTrip>> = Vec::new();
    // person -> (trip seq, index into individ_trips)
    let mut trips_per_person: MultiMap<(usize, usize), ((usize, bool, usize), usize)> =
        MultiMap::new();
    timer.start_iter("turn Soundcast trips into SpawnTrips", popdat.trips.len());
    for orig_trip in popdat.trips {
        timer.next();
        let trip = SpawnTrip::Remote {
            from: OffMapLocation {
                gps: orig_trip.from.pos,
                parcel_id: orig_trip.from.parcel_id,
            },
            to: OffMapLocation {
                gps: orig_trip.to.pos,
                parcel_id: orig_trip.to.parcel_id,
            },
            trip_time: orig_trip.trip_time,
            mode: orig_trip.mode,
        };
        let idx = individ_trips.len();
        individ_trips.push(Some(IndividTrip {
            depart: orig_trip.depart_at,
            trip,
        }));
        trips_per_person.insert(orig_trip.person, (orig_trip.seq, idx));
    }

    timer.note(format!(
        "{} trips over {} people",
        prettyprint_usize(individ_trips.len()),
        prettyprint_usize(trips_per_person.len())
    ));

    let mut people = Vec::new();
    for (orig_id, seq_trips) in trips_per_person.consume() {
        let id = PersonID(people.len());
        let mut trips = Vec::new();
        for (_, idx) in seq_trips {
            trips.push(individ_trips[idx].take().unwrap());
        }
        // Actually, the sequence in the Soundcast dataset crosses midnight. Don't do that; sort by
        // departure time starting with midnight.
        trips.sort_by_key(|t| t.depart);

        people.push(PersonSpec { id, orig_id, trips });
    }
    for maybe_t in individ_trips {
        if maybe_t.is_some() {
            panic!("Some IndividTrip wasn't associated with a Person?!");
        }
    }

    Scenario {
        scenario_name: "everyone_weekday".to_string(),
        map_name: map.get_name().to_string(),
        people,
        only_seed_buses: None,
    }
    .remove_weird_schedules(map)
}
