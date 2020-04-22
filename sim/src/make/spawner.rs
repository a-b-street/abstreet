use crate::{
    Command, CreateCar, CreatePedestrian, DrivingGoal, PersonID, Scheduler, SidewalkPOI,
    SidewalkSpot, TripEndpoint, TripLeg, TripManager, VehicleSpec, BIKE_LENGTH, MAX_CAR_LENGTH,
};
use abstutil::Timer;
use geom::{Speed, Time, EPSILON_DIST};
use map_model::{BuildingID, BusRouteID, BusStopID, Map, PathConstraints, PathRequest, Position};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TripSpec {
    // Can be used to spawn from a border or anywhere for interactive debugging.
    CarAppearing {
        start_pos: Position,
        goal: DrivingGoal,
        vehicle_spec: VehicleSpec,
        ped_speed: Speed,
    },
    MaybeUsingParkedCar {
        start_bldg: BuildingID,
        goal: DrivingGoal,
        ped_speed: Speed,
    },
    JustWalking {
        start: SidewalkSpot,
        goal: SidewalkSpot,
        ped_speed: Speed,
    },
    UsingBike {
        start: SidewalkSpot,
        goal: DrivingGoal,
        vehicle: VehicleSpec,
        ped_speed: Speed,
    },
    UsingTransit {
        start: SidewalkSpot,
        goal: SidewalkSpot,
        route: BusRouteID,
        stop1: BusStopID,
        stop2: BusStopID,
        ped_speed: Speed,
    },
}

// This structure is created temporarily by a Scenario or to interactively spawn agents.
pub struct TripSpawner {
    trips: Vec<(PersonID, Time, TripSpec)>,
}

impl TripSpawner {
    pub fn new() -> TripSpawner {
        TripSpawner { trips: Vec::new() }
    }

    pub fn schedule_trip(&mut self, person: PersonID, start_time: Time, spec: TripSpec, map: &Map) {
        // TODO We'll want to repeat this validation when we spawn stuff later for a second leg...
        match &spec {
            TripSpec::CarAppearing {
                start_pos,
                vehicle_spec,
                goal,
                ..
            } => {
                if start_pos.dist_along() < vehicle_spec.length {
                    panic!(
                        "Can't spawn a {:?} at {}; too close to the start",
                        vehicle_spec.vehicle_type,
                        start_pos.dist_along()
                    );
                }
                if start_pos.dist_along() >= map.get_l(start_pos.lane()).length() {
                    panic!(
                        "Can't spawn a {:?} at {}; {} isn't that long",
                        vehicle_spec.vehicle_type,
                        start_pos.dist_along(),
                        start_pos.lane()
                    );
                }
                match goal {
                    DrivingGoal::Border(_, end_lane) => {
                        if start_pos.lane() == *end_lane
                            && start_pos.dist_along() == map.get_l(*end_lane).length()
                        {
                            panic!(
                                "Can't start a {:?} at the edge of a border already",
                                vehicle_spec.vehicle_type
                            );
                        }
                    }
                    DrivingGoal::ParkNear(_) => {}
                }
            }
            TripSpec::MaybeUsingParkedCar { .. } => {}
            TripSpec::JustWalking { start, goal, .. } => {
                if start == goal {
                    panic!(
                        "A trip just walking from {:?} to {:?} doesn't make sense",
                        start, goal
                    );
                }
            }
            TripSpec::UsingBike {
                start,
                goal,
                ped_speed,
                ..
            } => {
                // TODO These trips are just silently erased; they don't even show up as aborted
                // trips! Really need to fix the underlying problem.
                if SidewalkSpot::bike_from_bike_rack(start.sidewalk_pos.lane(), map).is_none() {
                    println!(
                        "Can't start biking from {}; no biking or driving lane nearby?",
                        start.sidewalk_pos.lane()
                    );
                    return;
                }
                if let DrivingGoal::ParkNear(b) = goal {
                    let last_lane = goal.goal_pos(PathConstraints::Bike, map).lane();
                    // If bike_to_sidewalk works, then SidewalkSpot::bike_rack should too.
                    if map
                        .get_parent(last_lane)
                        .bike_to_sidewalk(last_lane)
                        .is_none()
                    {
                        println!(
                            "Can't fulfill {:?} for a bike trip; no sidewalk near {}",
                            goal, last_lane
                        );
                        return;
                    }
                    // A bike trip going from one lane to the same lane should... just walk.
                    if start.sidewalk_pos.lane() == map.get_b(*b).sidewalk() {
                        println!(
                            "Bike trip from {:?} to {:?} will just walk; it's the same sidewalk!",
                            start, goal
                        );
                        self.trips.push((
                            person,
                            start_time,
                            TripSpec::JustWalking {
                                start: start.clone(),
                                goal: SidewalkSpot::building(*b, map),
                                ped_speed: *ped_speed,
                            },
                        ));
                        return;
                    }
                }
            }
            TripSpec::UsingTransit { .. } => {}
        };

        self.trips.push((person, start_time, spec));
    }

    pub fn finalize(
        mut self,
        map: &Map,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        timer: &mut Timer,
        retry_if_no_room: bool,
    ) {
        let paths = timer.parallelize(
            "calculate paths",
            std::mem::replace(&mut self.trips, Vec::new()),
            |tuple| {
                let req = tuple.2.get_pathfinding_request(map);
                (tuple, req.clone(), map.pathfind(req))
            },
        );

        timer.start_iter("spawn trips", paths.len());
        for ((person, start_time, spec), req, maybe_path) in paths {
            timer.next();
            match spec {
                TripSpec::CarAppearing {
                    start_pos,
                    vehicle_spec,
                    goal,
                    ped_speed,
                } => {
                    let vehicle = vehicle_spec.make(trips.new_car_id(), Some(person));
                    let mut legs = vec![TripLeg::Drive(vehicle.clone(), goal.clone())];
                    if let DrivingGoal::ParkNear(b) = goal {
                        legs.push(TripLeg::Walk(
                            trips.new_ped_id(),
                            ped_speed,
                            SidewalkSpot::building(b, map),
                        ));
                    }
                    let trip_start = TripEndpoint::Border(map.get_l(start_pos.lane()).src_i);
                    let trip = trips.new_trip(person, start_time, trip_start, legs);
                    if let Some(path) = maybe_path {
                        let router = goal.make_router(path, map, vehicle.vehicle_type);
                        scheduler.push(
                            start_time,
                            Command::SpawnCar(
                                CreateCar::for_appearing(
                                    vehicle, start_pos, router, req, trip, person,
                                ),
                                retry_if_no_room,
                            ),
                        );
                    } else {
                        timer.warn(format!(
                            "CarAppearing trip couldn't find the first path {}",
                            req
                        ));
                        trips.abort_trip_failed_start(trip);
                    }
                }
                TripSpec::MaybeUsingParkedCar {
                    start_bldg,
                    goal,
                    ped_speed,
                } => {
                    let walk_to = SidewalkSpot::deferred_parking_spot(start_bldg, goal, map);
                    // Can't add TripLeg::Drive, because we don't know the vehicle yet! Plumb along
                    // the DrivingGoal, so we can expand the trip later.
                    let id = trips.new_ped_id();
                    let legs = vec![TripLeg::Walk(id, ped_speed, walk_to.clone())];
                    let trip =
                        trips.new_trip(person, start_time, TripEndpoint::Bldg(start_bldg), legs);

                    scheduler.push(
                        start_time,
                        Command::SpawnPed(CreatePedestrian {
                            id,
                            speed: ped_speed,
                            start: SidewalkSpot::building(start_bldg, map),
                            goal: walk_to,
                            // This is guaranteed to work, and is junk anyway.
                            path: maybe_path.unwrap(),
                            req,
                            trip,
                            person,
                        }),
                    );
                }
                TripSpec::JustWalking {
                    start,
                    goal,
                    ped_speed,
                } => {
                    let id = trips.new_ped_id();
                    let trip = trips.new_trip(
                        person,
                        start_time,
                        match start.connection {
                            SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                            SidewalkPOI::SuddenlyAppear => {
                                TripEndpoint::Border(map.get_l(start.sidewalk_pos.lane()).src_i)
                            }
                            SidewalkPOI::Border(i) => TripEndpoint::Border(i),
                            _ => unreachable!(),
                        },
                        vec![TripLeg::Walk(id, ped_speed, goal.clone())],
                    );

                    if let Some(path) = maybe_path {
                        scheduler.push(
                            start_time,
                            Command::SpawnPed(CreatePedestrian {
                                id,
                                speed: ped_speed,
                                start,
                                goal,
                                path,
                                req,
                                trip,
                                person,
                            }),
                        );
                    } else {
                        timer.warn(format!(
                            "JustWalking trip couldn't find the first path {}",
                            req
                        ));
                        trips.abort_trip_failed_start(trip);
                    }
                }
                TripSpec::UsingBike {
                    start,
                    vehicle,
                    goal,
                    ped_speed,
                } => {
                    let ped_id = trips.new_ped_id();
                    let bike_id = trips.new_car_id();
                    let walk_to =
                        SidewalkSpot::bike_from_bike_rack(start.sidewalk_pos.lane(), map).unwrap();
                    let mut legs = vec![
                        TripLeg::Walk(ped_id, ped_speed, walk_to.clone()),
                        TripLeg::Drive(vehicle.make(bike_id, None), goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(
                                ped_id,
                                ped_speed,
                                SidewalkSpot::building(b, map),
                            ));
                        }
                        DrivingGoal::Border(_, _) => {}
                    };
                    let trip = trips.new_trip(
                        person,
                        start_time,
                        match start.connection {
                            SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                            SidewalkPOI::SuddenlyAppear => {
                                TripEndpoint::Border(map.get_l(start.sidewalk_pos.lane()).src_i)
                            }
                            SidewalkPOI::Border(i) => TripEndpoint::Border(i),
                            _ => unreachable!(),
                        },
                        legs,
                    );

                    if let Some(path) = maybe_path {
                        scheduler.push(
                            start_time,
                            Command::SpawnPed(CreatePedestrian {
                                id: ped_id,
                                speed: ped_speed,
                                start,
                                goal: walk_to,
                                path,
                                req,
                                trip,
                                person,
                            }),
                        );
                    } else {
                        timer.warn(format!(
                            "UsingBike trip couldn't find the first path {}",
                            req
                        ));
                        trips.abort_trip_failed_start(trip);
                    }
                }
                TripSpec::UsingTransit {
                    start,
                    route,
                    stop1,
                    stop2,
                    goal,
                    ped_speed,
                } => {
                    let id = trips.new_ped_id();
                    let walk_to = SidewalkSpot::bus_stop(stop1, map);
                    let trip = trips.new_trip(
                        person,
                        start_time,
                        match start.connection {
                            SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                            SidewalkPOI::SuddenlyAppear => {
                                TripEndpoint::Border(map.get_l(start.sidewalk_pos.lane()).src_i)
                            }
                            SidewalkPOI::Border(i) => TripEndpoint::Border(i),
                            _ => unreachable!(),
                        },
                        vec![
                            TripLeg::Walk(id, ped_speed, walk_to.clone()),
                            TripLeg::RideBus(id, route, stop2),
                            TripLeg::Walk(id, ped_speed, goal),
                        ],
                    );

                    if let Some(path) = maybe_path {
                        scheduler.push(
                            start_time,
                            Command::SpawnPed(CreatePedestrian {
                                id,
                                speed: ped_speed,
                                start,
                                goal: walk_to,
                                path,
                                req,
                                trip,
                                person,
                            }),
                        );
                    } else {
                        timer.warn(format!(
                            "UsingTransit trip couldn't find the first path {}",
                            req
                        ));
                        trips.abort_trip_failed_start(trip);
                    }
                }
            }
        }
    }
}

impl TripSpec {
    // If possible, fixes problems that schedule_trip would hit.
    pub fn spawn_vehicle_at(pos: Position, is_bike: bool, map: &Map) -> Option<Position> {
        let lane_len = map.get_l(pos.lane()).length();
        let vehicle_len = if is_bike { BIKE_LENGTH } else { MAX_CAR_LENGTH };
        // There's no hope.
        if lane_len <= vehicle_len {
            return None;
        }

        if pos.dist_along() < vehicle_len {
            Some(Position::new(pos.lane(), vehicle_len))
        } else if pos.dist_along() == lane_len {
            Some(Position::new(pos.lane(), pos.dist_along() - EPSILON_DIST))
        } else {
            Some(pos)
        }
    }

    pub(crate) fn get_pathfinding_request(&self, map: &Map) -> PathRequest {
        match self {
            TripSpec::CarAppearing {
                start_pos,
                vehicle_spec,
                goal,
                ..
            } => {
                let constraints = vehicle_spec.vehicle_type.to_constraints();
                PathRequest {
                    start: *start_pos,
                    end: goal.goal_pos(constraints, map),
                    constraints,
                }
            }
            // Don't know where the parked car will be, so just make a dummy path that'll never
            // fail.
            TripSpec::MaybeUsingParkedCar { start_bldg, .. } => {
                let pos = map.get_b(*start_bldg).front_path.sidewalk;
                PathRequest {
                    start: pos,
                    end: pos,
                    constraints: PathConstraints::Pedestrian,
                }
            }
            TripSpec::JustWalking { start, goal, .. } => PathRequest {
                start: start.sidewalk_pos,
                end: goal.sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            },
            TripSpec::UsingBike { start, .. } => PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::bike_from_bike_rack(start.sidewalk_pos.lane(), map)
                    .unwrap()
                    .sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            },
            TripSpec::UsingTransit { start, stop1, .. } => PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::bus_stop(*stop1, map).sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            },
        }
    }
}
