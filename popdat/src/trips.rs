use crate::psrc::{Endpoint, Mode, Parcel, Purpose};
use crate::PopDat;
use abstutil::Timer;
use geom::{Distance, Duration, LonLat, Polygon, Pt2D, Time};
use map_model::{BuildingID, IntersectionID, Map, PathConstraints, Position};
use sim::{DrivingGoal, Scenario, SidewalkSpot, SpawnTrip, TripSpec};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
pub struct Trip {
    pub from: TripEndpt,
    pub to: TripEndpt,
    pub depart_at: Time,
    pub purpose: (Purpose, Purpose),
    pub mode: Mode,
    // These are an upper bound when TripEndpt::Border is involved.
    pub trip_time: Duration,
    pub trip_dist: Distance,
}

#[derive(Clone, Debug)]
pub enum TripEndpt {
    Building(BuildingID),
    // The Pt2D is the original point. It'll be outside the map and likely out-of-bounds entirely,
    // maybe even negative.
    Border(IntersectionID, Pt2D),
}

impl Trip {
    pub fn end_time(&self) -> Time {
        self.depart_at + self.trip_time
    }

    pub fn to_spawn_trip(&self, map: &Map) -> Option<SpawnTrip> {
        match self.mode {
            Mode::Drive => match self.from {
                TripEndpt::Border(i, _) => {
                    if let Some(start) = TripSpec::spawn_car_at(
                        Position::new(
                            map.get_i(i).get_outgoing_lanes(map, PathConstraints::Car)[0],
                            Distance::ZERO,
                        ),
                        map,
                    ) {
                        Some(SpawnTrip::CarAppearing {
                            depart: self.depart_at,
                            start,
                            goal: self.to.driving_goal(PathConstraints::Car, map),
                            is_bike: false,
                        })
                    } else {
                        // TODO need to be able to emit warnings from parallelize
                        //timer.warn(format!("No room for car to appear at {:?}", self.from));
                        None
                    }
                }
                TripEndpt::Building(b) => Some(SpawnTrip::MaybeUsingParkedCar(
                    self.depart_at,
                    b,
                    self.to.driving_goal(PathConstraints::Car, map),
                )),
            },
            Mode::Bike => match self.from {
                TripEndpt::Building(b) => Some(SpawnTrip::UsingBike(
                    self.depart_at,
                    SidewalkSpot::building(b, map),
                    self.to.driving_goal(PathConstraints::Bike, map),
                )),
                TripEndpt::Border(i, _) => {
                    if let Some(start) = TripSpec::spawn_car_at(
                        Position::new(
                            map.get_i(i).get_outgoing_lanes(map, PathConstraints::Bike)[0],
                            Distance::ZERO,
                        ),
                        map,
                    ) {
                        Some(SpawnTrip::CarAppearing {
                            depart: self.depart_at,
                            start,
                            goal: self.to.driving_goal(PathConstraints::Bike, map),
                            is_bike: true,
                        })
                    } else {
                        //timer.warn(format!("No room for bike to appear at {:?}", self.from));
                        None
                    }
                }
            },
            Mode::Walk => Some(SpawnTrip::JustWalking(
                self.depart_at,
                self.from.start_sidewalk_spot(map),
                self.to.end_sidewalk_spot(map),
            )),
            Mode::Transit => {
                let start = self.from.start_sidewalk_spot(map);
                let goal = self.to.end_sidewalk_spot(map);
                if let Some((stop1, stop2, route)) =
                    map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
                {
                    Some(SpawnTrip::UsingTransit(
                        self.depart_at,
                        start,
                        goal,
                        route,
                        stop1,
                        stop2,
                    ))
                } else {
                    //timer.warn(format!("{:?} not actually using transit, because pathfinding didn't find any useful route", trip));
                    Some(SpawnTrip::JustWalking(self.depart_at, start, goal))
                }
            }
        }
    }
}

impl TripEndpt {
    fn new(
        endpt: &Endpoint,
        map: &Map,
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
                    Pt2D::forcibly_from_gps(endpt.pos, map.get_gps_bounds()),
                )
            })
    }

    fn start_sidewalk_spot(&self, map: &Map) -> SidewalkSpot {
        match self {
            TripEndpt::Building(b) => SidewalkSpot::building(*b, map),
            TripEndpt::Border(i, _) => SidewalkSpot::start_at_border(*i, map).unwrap(),
        }
    }

    fn end_sidewalk_spot(&self, map: &Map) -> SidewalkSpot {
        match self {
            TripEndpt::Building(b) => SidewalkSpot::building(*b, map),
            TripEndpt::Border(i, _) => SidewalkSpot::end_at_border(*i, map).unwrap(),
        }
    }

    fn driving_goal(&self, constraints: PathConstraints, map: &Map) -> DrivingGoal {
        match self {
            TripEndpt::Building(b) => DrivingGoal::ParkNear(*b),
            TripEndpt::Border(i, _) => {
                DrivingGoal::end_at_border(map.get_i(*i).some_incoming_road(map), constraints, map)
                    .unwrap()
            }
        }
    }

    pub fn polygon<'a>(&self, map: &'a Map) -> &'a Polygon {
        match self {
            TripEndpt::Building(b) => &map.get_b(*b).polygon,
            TripEndpt::Border(i, _) => &map.get_i(*i).polygon,
        }
    }
}

pub fn clip_trips(map: &Map, timer: &mut Timer) -> (Vec<Trip>, HashMap<BuildingID, Parcel>) {
    let popdat: PopDat = abstutil::read_binary("../data/shapes/popdat.bin", timer);

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

    let maybe_results: Vec<Option<Trip>> = timer.parallelize("clip trips", popdat.trips, |trip| {
        let from = TripEndpt::new(
            &trip.from,
            map,
            &osm_id_to_bldg,
            match trip.mode {
                Mode::Walk | Mode::Transit => &incoming_borders_walking,
                Mode::Drive => &incoming_borders_driving,
                Mode::Bike => &incoming_borders_biking,
            },
        )?;
        let to = TripEndpt::new(
            &trip.to,
            map,
            &osm_id_to_bldg,
            match trip.mode {
                Mode::Walk | Mode::Transit => &outgoing_borders_walking,
                Mode::Drive => &outgoing_borders_driving,
                Mode::Bike => &outgoing_borders_biking,
            },
        )?;

        let trip = Trip {
            from,
            to,
            depart_at: trip.depart_at,
            purpose: trip.purpose,
            mode: trip.mode,
            trip_time: trip.trip_time,
            trip_dist: trip.trip_dist,
        };

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
    let trips = maybe_results.into_iter().flatten().collect();

    let mut bldgs = HashMap::new();
    for (osm_id, metadata) in popdat.parcels {
        if let Some(b) = osm_id_to_bldg.get(&osm_id) {
            bldgs.insert(*b, metadata);
        }
    }
    (trips, bldgs)
}

pub fn trips_to_scenario(map: &Map, timer: &mut Timer) -> Scenario {
    let (trips, _) = clip_trips(map, timer);
    // TODO Don't clone trips for parallelize
    let individ_trips = timer
        .parallelize("turn PSRC trips into SpawnTrips", trips.clone(), |trip| {
            trip.to_spawn_trip(map)
        })
        .into_iter()
        .flatten()
        .collect();

    // How many parked cars do we need to spawn near each building?
    // TODO This assumes trips are instantaneous. At runtime, somebody might try to use a parked
    // car from a building, but one hasn't been delivered yet.
    let mut individ_parked_cars = BTreeMap::new();
    let mut avail_per_bldg = BTreeMap::new();
    for b in map.all_buildings() {
        individ_parked_cars.insert(b.id, 0);
        avail_per_bldg.insert(b.id, 0);
    }
    for trip in trips {
        if trip.mode != Mode::Drive {
            continue;
        }
        if let TripEndpt::Building(b) = trip.from {
            if avail_per_bldg[&b] > 0 {
                *avail_per_bldg.get_mut(&b).unwrap() -= 1;
            } else {
                *individ_parked_cars.get_mut(&b).unwrap() += 1;
            }
        }
        if let TripEndpt::Building(b) = trip.to {
            *avail_per_bldg.get_mut(&b).unwrap() += 1;
        }
    }

    Scenario {
        scenario_name: "weekday_typical_traffic_from_psrc".to_string(),
        map_name: map.get_name().to_string(),
        seed_buses: true,
        seed_parked_cars: Vec::new(),
        spawn_over_time: Vec::new(),
        border_spawn_over_time: Vec::new(),
        individ_trips,
        individ_parked_cars,
    }
}
