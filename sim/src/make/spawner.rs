use crate::{
    CarID, Command, DrivingGoal, OffMapLocation, Person, PersonID, Scheduler, SidewalkSpot,
    TripEndpoint, TripLeg, TripManager, TripMode, VehicleType, BIKE_LENGTH, MAX_CAR_LENGTH,
};
use abstutil::Timer;
use geom::{Duration, Time, EPSILON_DIST};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathConstraints, PathRequest, Position,
};
use serde_derive::{Deserialize, Serialize};

// TODO Some of these fields are unused now that we separately pass TripEndpoint
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TripSpec {
    // Can be used to spawn from a border or anywhere for interactive debugging.
    VehicleAppearing {
        start_pos: Position,
        goal: DrivingGoal,
        // This must be a currently off-map vehicle owned by the person.
        use_vehicle: CarID,
        retry_if_no_room: bool,
        origin: Option<OffMapLocation>,
    },
    // A VehicleAppearing that failed to even pick a start_pos, because of a bug with badly chosen
    // borders.
    NoRoomToSpawn {
        i: IntersectionID,
        goal: DrivingGoal,
        use_vehicle: CarID,
        origin: Option<OffMapLocation>,
    },
    UsingParkedCar {
        // This must be a currently parked vehicle owned by the person.
        car: CarID,
        start_bldg: BuildingID,
        goal: DrivingGoal,
    },
    JustWalking {
        start: SidewalkSpot,
        goal: SidewalkSpot,
    },
    UsingBike {
        bike: CarID,
        start: SidewalkSpot,
        goal: DrivingGoal,
    },
    UsingTransit {
        start: SidewalkSpot,
        goal: SidewalkSpot,
        route: BusRouteID,
        stop1: BusStopID,
        stop2: BusStopID,
    },
    // Completely off-map trip. Don't really simulate much of it.
    Remote {
        from: OffMapLocation,
        to: OffMapLocation,
        trip_time: Duration,
        mode: TripMode,
    },
}

// This structure is created temporarily by a Scenario or to interactively spawn agents.
pub struct TripSpawner {
    trips: Vec<(PersonID, Time, TripSpec, TripEndpoint)>,
}

impl TripSpawner {
    pub fn new() -> TripSpawner {
        TripSpawner { trips: Vec::new() }
    }

    pub fn schedule_trip(
        &mut self,
        person: &Person,
        start_time: Time,
        spec: TripSpec,
        trip_start: TripEndpoint,
        map: &Map,
    ) {
        // TODO We'll want to repeat this validation when we spawn stuff later for a second leg...
        match &spec {
            TripSpec::VehicleAppearing {
                start_pos,
                goal,
                use_vehicle,
                ..
            } => {
                let vehicle = person.get_vehicle(*use_vehicle);
                if start_pos.dist_along() < vehicle.length {
                    panic!(
                        "Can't spawn a {:?} at {}; too close to the start",
                        vehicle.vehicle_type,
                        start_pos.dist_along()
                    );
                }
                if start_pos.dist_along() >= map.get_l(start_pos.lane()).length() {
                    panic!(
                        "Can't spawn a {:?} at {}; {} isn't that long",
                        vehicle.vehicle_type,
                        start_pos.dist_along(),
                        start_pos.lane()
                    );
                }
                match goal {
                    DrivingGoal::Border(_, end_lane, _) => {
                        if start_pos.lane() == *end_lane
                            && start_pos.dist_along() == map.get_l(*end_lane).length()
                        {
                            panic!(
                                "Can't start a {:?} at the edge of a border already",
                                vehicle.vehicle_type
                            );
                        }
                    }
                    DrivingGoal::ParkNear(_) => {}
                }
            }
            TripSpec::NoRoomToSpawn { .. } => {}
            TripSpec::UsingParkedCar { .. } => {}
            TripSpec::JustWalking { start, goal, .. } => {
                if start == goal {
                    panic!(
                        "A trip just walking from {:?} to {:?} doesn't make sense",
                        start, goal
                    );
                }
            }
            TripSpec::UsingBike { start, goal, .. } => {
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
                            person.id,
                            start_time,
                            TripSpec::JustWalking {
                                start: start.clone(),
                                goal: SidewalkSpot::building(*b, map),
                            },
                            trip_start,
                        ));
                        return;
                    }
                }
            }
            TripSpec::UsingTransit { .. } => {}
            TripSpec::Remote { .. } => {}
        };

        self.trips.push((person.id, start_time, spec, trip_start));
    }

    pub fn finalize(
        mut self,
        map: &Map,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        timer: &mut Timer,
    ) {
        let paths = timer.parallelize(
            "calculate paths",
            std::mem::replace(&mut self.trips, Vec::new()),
            |tuple| {
                let req = tuple.2.get_pathfinding_request(map);
                (tuple, req.clone(), req.and_then(|r| map.pathfind(r)))
            },
        );

        timer.start_iter("spawn trips", paths.len());
        for ((p, start_time, spec, trip_start), maybe_req, maybe_path) in paths {
            timer.next();

            // TODO clone() is super weird to do here, but we just need to make the borrow checker
            // happy. All we're doing is grabbing IDs off this.
            let person = trips.get_person(p).unwrap().clone();
            // Just create the trip for each case.
            // TODO Not happy about this clone()
            let trip = match spec.clone() {
                TripSpec::VehicleAppearing {
                    goal, use_vehicle, ..
                } => {
                    let mut legs = vec![TripLeg::Drive(use_vehicle, goal.clone())];
                    if let DrivingGoal::ParkNear(b) = goal {
                        legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                    }
                    trips.new_trip(
                        person.id,
                        start_time,
                        trip_start,
                        if use_vehicle.1 == VehicleType::Bike {
                            TripMode::Bike
                        } else {
                            TripMode::Drive
                        },
                        legs,
                        map,
                    )
                }
                TripSpec::NoRoomToSpawn {
                    goal, use_vehicle, ..
                } => {
                    let mut legs = vec![TripLeg::Drive(use_vehicle, goal.clone())];
                    if let DrivingGoal::ParkNear(b) = goal {
                        legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                    }
                    trips.new_trip(
                        person.id,
                        start_time,
                        trip_start,
                        if use_vehicle.1 == VehicleType::Bike {
                            TripMode::Bike
                        } else {
                            TripMode::Drive
                        },
                        legs,
                        map,
                    )
                }
                TripSpec::UsingParkedCar { car, goal, .. } => {
                    let mut legs = vec![
                        TripLeg::Walk(SidewalkSpot::deferred_parking_spot()),
                        TripLeg::Drive(car, goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                        }
                        DrivingGoal::Border(_, _, _) => {}
                    }
                    trips.new_trip(
                        person.id,
                        start_time,
                        trip_start,
                        TripMode::Drive,
                        legs,
                        map,
                    )
                }
                TripSpec::JustWalking { goal, .. } => trips.new_trip(
                    person.id,
                    start_time,
                    trip_start,
                    TripMode::Walk,
                    vec![TripLeg::Walk(goal.clone())],
                    map,
                ),
                TripSpec::UsingBike { bike, start, goal } => {
                    let walk_to =
                        SidewalkSpot::bike_from_bike_rack(start.sidewalk_pos.lane(), map).unwrap();
                    let mut legs = vec![
                        TripLeg::Walk(walk_to.clone()),
                        TripLeg::Drive(bike, goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                        }
                        DrivingGoal::Border(_, _, _) => {}
                    };
                    trips.new_trip(person.id, start_time, trip_start, TripMode::Bike, legs, map)
                }
                TripSpec::UsingTransit {
                    route,
                    stop1,
                    stop2,
                    goal,
                    ..
                } => {
                    let walk_to = SidewalkSpot::bus_stop(stop1, map);
                    trips.new_trip(
                        person.id,
                        start_time,
                        trip_start,
                        TripMode::Transit,
                        vec![
                            TripLeg::Walk(walk_to.clone()),
                            TripLeg::RideBus(route, stop2),
                            TripLeg::Walk(goal),
                        ],
                        map,
                    )
                }
                TripSpec::Remote { to, mode, .. } => trips.new_trip(
                    person.id,
                    start_time,
                    trip_start,
                    mode,
                    vec![TripLeg::Remote(to)],
                    map,
                ),
            };
            scheduler.push(
                start_time,
                Command::StartTrip(trip, spec, maybe_req, maybe_path),
            );
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

    pub(crate) fn get_pathfinding_request(&self, map: &Map) -> Option<PathRequest> {
        match self {
            TripSpec::VehicleAppearing {
                start_pos,
                goal,
                use_vehicle,
                ..
            } => {
                let constraints = if use_vehicle.1 == VehicleType::Bike {
                    PathConstraints::Bike
                } else {
                    PathConstraints::Car
                };
                Some(PathRequest {
                    start: *start_pos,
                    end: goal.goal_pos(constraints, map),
                    constraints,
                })
            }
            TripSpec::NoRoomToSpawn { .. } => None,
            // We don't know where the parked car will be
            TripSpec::UsingParkedCar { .. } => None,
            TripSpec::JustWalking { start, goal, .. } => Some(PathRequest {
                start: start.sidewalk_pos,
                end: goal.sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            }),
            TripSpec::UsingBike { start, .. } => Some(PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::bike_from_bike_rack(start.sidewalk_pos.lane(), map)
                    .unwrap()
                    .sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            }),
            TripSpec::UsingTransit { start, stop1, .. } => Some(PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::bus_stop(*stop1, map).sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            }),
            TripSpec::Remote { .. } => None,
        }
    }
}
