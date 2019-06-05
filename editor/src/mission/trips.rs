use crate::ui::UI;
use abstutil::Timer;
use ezgui::EventCtx;
use geom::{Distance, Duration, LonLat, PolyLine, Polygon, Pt2D};
use map_model::{BuildingID, IntersectionID, LaneType, Map, PathRequest, Position};
use sim::{DrivingGoal, SidewalkSpot};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Trip {
    pub from: TripEndpt,
    pub to: TripEndpt,
    pub depart_at: Duration,
    pub purpose: (popdat::psrc::Purpose, popdat::psrc::Purpose),
    pub mode: popdat::psrc::Mode,
    // These are an upper bound when TripEndpt::Border is involved.
    pub trip_time: Duration,
    pub trip_dist: Distance,
    // clip_trips doesn't populate this.
    pub route: Option<PolyLine>,
}

#[derive(Debug)]
pub enum TripEndpt {
    Building(BuildingID),
    // The Pt2D is the original point. It'll be outside the map and likely out-of-bounds entirely,
    // maybe even negative.
    Border(IntersectionID, Pt2D),
}

impl Trip {
    pub fn end_time(&self) -> Duration {
        self.depart_at + self.trip_time
    }

    pub fn path_req(&self, map: &Map) -> PathRequest {
        use popdat::psrc::Mode;

        match self.mode {
            Mode::Walk => PathRequest {
                start: self.from.start_sidewalk_spot(map).sidewalk_pos,
                end: self.from.end_sidewalk_spot(map).sidewalk_pos,
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            Mode::Bike => PathRequest {
                start: self.from.start_pos_driving(map),
                end: self
                    .to
                    .driving_goal(vec![LaneType::Biking, LaneType::Driving], map)
                    .goal_pos(map),
                can_use_bike_lanes: true,
                can_use_bus_lanes: false,
            },
            Mode::Drive => PathRequest {
                start: self.from.start_pos_driving(map),
                end: self
                    .to
                    .driving_goal(vec![LaneType::Driving], map)
                    .goal_pos(map),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            Mode::Transit => {
                let start = self.from.start_sidewalk_spot(map).sidewalk_pos;
                let end = self.to.end_sidewalk_spot(map).sidewalk_pos;
                if let Some((stop1, _, _)) = map.should_use_transit(start, end) {
                    PathRequest {
                        start,
                        end: SidewalkSpot::bus_stop(stop1, map).sidewalk_pos,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: false,
                    }
                } else {
                    // Just fall back to walking. :\
                    PathRequest {
                        start,
                        end,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: false,
                    }
                }
            }
        }
    }
}

impl TripEndpt {
    fn new(
        endpt: &popdat::psrc::Endpoint,
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

    // TODO or biking
    // TODO bldg_via_driving needs to do find_driving_lane_near_building sometimes
    // Doesn't adjust for starting length yet.
    fn start_pos_driving(&self, map: &Map) -> Position {
        match self {
            TripEndpt::Building(b) => Position::bldg_via_driving(*b, map).unwrap(),
            TripEndpt::Border(i, _) => {
                let lane = map.get_i(*i).get_outgoing_lanes(map, LaneType::Driving)[0];
                Position::new(lane, Distance::ZERO)
            }
        }
    }

    fn driving_goal(&self, lane_types: Vec<LaneType>, map: &Map) -> DrivingGoal {
        match self {
            TripEndpt::Building(b) => DrivingGoal::ParkNear(*b),
            TripEndpt::Border(i, _) => DrivingGoal::end_at_border(*i, lane_types, map).unwrap(),
        }
    }

    pub fn polygon<'a>(&self, map: &'a Map) -> &'a Polygon {
        match self {
            TripEndpt::Building(b) => &map.get_b(*b).polygon,
            TripEndpt::Border(i, _) => &map.get_i(*i).polygon,
        }
    }
}

pub fn clip_trips(ui: &UI, timer: &mut Timer) -> Vec<Trip> {
    use popdat::psrc::Mode;

    let popdat: popdat::PopDat =
        abstutil::read_binary("../data/shapes/popdat", timer).expect("Couldn't load popdat");

    let map = &ui.primary.map;

    let mut osm_id_to_bldg = HashMap::new();
    for b in map.all_buildings() {
        osm_id_to_bldg.insert(b.osm_way_id, b.id);
    }
    let bounds = map.get_gps_bounds();
    let incoming_borders_walking: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| !i.get_outgoing_lanes(map, LaneType::Sidewalk).is_empty())
        .map(|i| (i.id, i.point.to_gps(bounds).unwrap()))
        .collect();
    let incoming_borders_driving: Vec<(IntersectionID, LonLat)> = map
        .all_incoming_borders()
        .into_iter()
        .filter(|i| !i.get_outgoing_lanes(map, LaneType::Driving).is_empty())
        .map(|i| (i.id, i.point.to_gps(bounds).unwrap()))
        .collect();
    let outgoing_borders_walking: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| !i.get_incoming_lanes(map, LaneType::Sidewalk).is_empty())
        .map(|i| (i.id, i.point.to_gps(bounds).unwrap()))
        .collect();
    let outgoing_borders_driving: Vec<(IntersectionID, LonLat)> = map
        .all_outgoing_borders()
        .into_iter()
        .filter(|i| !i.get_incoming_lanes(map, LaneType::Driving).is_empty())
        .map(|i| (i.id, i.point.to_gps(bounds).unwrap()))
        .collect();

    let maybe_results: Vec<Option<Trip>> = timer.parallelize("clip trips", popdat.trips, |trip| {
        let from = TripEndpt::new(
            &trip.from,
            map,
            &osm_id_to_bldg,
            match trip.mode {
                Mode::Walk | Mode::Transit => &incoming_borders_walking,
                Mode::Drive | Mode::Bike => &incoming_borders_driving,
            },
        )?;
        let to = TripEndpt::new(
            &trip.to,
            map,
            &osm_id_to_bldg,
            match trip.mode {
                Mode::Walk | Mode::Transit => &outgoing_borders_walking,
                Mode::Drive | Mode::Bike => &outgoing_borders_driving,
            },
        )?;

        let mut trip = Trip {
            from,
            to,
            depart_at: trip.depart_at,
            purpose: trip.purpose,
            mode: trip.mode,
            trip_time: trip.trip_time,
            trip_dist: trip.trip_dist,
            route: None,
        };

        match (&trip.from, &trip.to) {
            (TripEndpt::Border(_, _), TripEndpt::Border(_, _)) => {
                // TODO Detect and handle pass-through trips
                return None;
            }
            // Fix depart_at, trip_time, and trip_dist for border cases. Assume constant speed
            // through the trip.
            // TODO Disabled because slow and nonsensical distance ratios. :(
            (TripEndpt::Border(_, _), TripEndpt::Building(_)) => {
                if false {
                    // TODO Figure out why some paths fail.
                    // TODO Since we're doing the work anyway, store the result?
                    let dist = map.pathfind(trip.path_req(map))?.total_dist(map);
                    // TODO This is failing all over the place, why?
                    assert!(dist <= trip.trip_dist);
                    let trip_time = (dist / trip.trip_dist) * trip.trip_time;
                    trip.depart_at += trip.trip_time - trip_time;
                    trip.trip_time = trip_time;
                    trip.trip_dist = dist;
                }
            }
            (TripEndpt::Building(_), TripEndpt::Border(_, _)) => {
                if false {
                    let dist = map.pathfind(trip.path_req(map))?.total_dist(map);
                    assert!(dist <= trip.trip_dist);
                    trip.trip_time = (dist / trip.trip_dist) * trip.trip_time;
                    trip.trip_dist = dist;
                }
            }
            (TripEndpt::Building(_), TripEndpt::Building(_)) => {}
        }

        Some(trip)
    });
    maybe_results.into_iter().flatten().collect()
}

pub fn instantiate_trips(ctx: &mut EventCtx, ui: &mut UI) {
    use popdat::psrc::Mode;
    use sim::{Scenario, TripSpec};

    ctx.loading_screen("set up sim with PSRC trips", |_, mut timer| {
        let map = &ui.primary.map;
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();

        let mut min_time = Duration::parse("23:59:59.9").unwrap();

        let trips = clip_trips(ui, &mut timer);
        // TODO parallelize this -- except timer.warn and rng aren't threadsafe.
        timer.start_iter("turn PSRC trips into sim trips", trips.len());
        for trip in trips {
            timer.next();
            ui.primary.sim.schedule_trip(
                trip.depart_at,
                match trip.mode {
                    // TODO Use a parked car, but first have to figure out what cars to seed.
                    Mode::Drive => {
                        if let Some(start_pos) = TripSpec::spawn_car_at(
                            trip.from.start_pos_driving(map),
                            map,
                        ) {
                            TripSpec::CarAppearing {
                                start_pos,
                                goal: trip.to.driving_goal(vec![LaneType::Driving], map),
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                                vehicle_spec: Scenario::rand_car(&mut rng),
                            }
                        } else {
                            timer.warn(format!("No room for car to appear at {:?}", trip.from));
                            continue;
                        }
                    }
                    Mode::Bike => match trip.from {
                        TripEndpt::Building(b) => TripSpec::UsingBike {
                            start: SidewalkSpot::building(b, map),
                            goal: trip.to.driving_goal(vec![LaneType::Biking, LaneType::Driving], map),
                            ped_speed: Scenario::rand_ped_speed(&mut rng),
                            vehicle: Scenario::rand_bike(&mut rng),
                        },
                        TripEndpt::Border(_, _) => {
                            if let Some(start_pos) = TripSpec::spawn_car_at(
                                trip.from.start_pos_driving(map),
                                map,
                            ) {
                                TripSpec::CarAppearing {
                                    start_pos,
                                    goal: trip.to.driving_goal(vec![LaneType::Biking, LaneType::Driving], map),
                                    ped_speed: Scenario::rand_ped_speed(&mut rng),
                                    vehicle_spec: Scenario::rand_bike(&mut rng),
                                }
                            } else {
                                timer.warn(format!("No room for bike to appear at {:?}", trip.from));
                                continue;
                            }
                        },
                    },
                    Mode::Walk => TripSpec::JustWalking {
                        start: trip.from.start_sidewalk_spot(map),
                        goal: trip.to.end_sidewalk_spot(map),
                        ped_speed: Scenario::rand_ped_speed(&mut rng),
                    },
                    Mode::Transit => {
                        let start = trip.from.start_sidewalk_spot(map);
                        let goal = trip.to.end_sidewalk_spot(map);
                        let ped_speed = Scenario::rand_ped_speed(&mut rng);
                        if let Some((stop1, stop2, route)) = map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos) {
                            TripSpec::UsingTransit {
                                start, goal, route, stop1, stop2, ped_speed,
                            }
                        } else {
                            timer.warn(format!("{:?} not actually using transit, because pathfinding didn't find any useful route", trip));
                            TripSpec::JustWalking {
                                start, goal, ped_speed }
                        }
                    }
                },
                map,
            );
            min_time = min_time.min(trip.depart_at);
        }
        timer.note(format!("Expect the first trip to start at {}", min_time));

        for route in map.get_all_bus_routes() {
            ui.primary.sim.seed_bus_route(route, map, &mut timer);
        }

        ui.primary.sim.spawn_all_trips(map, &mut timer, true);
        ui.primary.sim.step(map, Duration::const_seconds(0.1));
    });
}
