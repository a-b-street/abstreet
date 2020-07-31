use crate::{
    CarID, Command, DrivingGoal, OffMapLocation, Person, PersonID, Scheduler, SidewalkSpot,
    TripEndpoint, TripLeg, TripManager, TripMode, VehicleType,
};
use abstutil::{Parallelism, Timer};
use geom::{Duration, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathConstraints, PathRequest, Position,
};
use serde::{Deserialize, Serialize};

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
        error: String,
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
        start: BuildingID,
        goal: DrivingGoal,
    },
    UsingTransit {
        start: SidewalkSpot,
        goal: SidewalkSpot,
        route: BusRouteID,
        stop1: BusStopID,
        maybe_stop2: Option<BusStopID>,
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
    trips: Vec<(PersonID, Time, TripSpec, TripEndpoint, bool, bool)>,
}

impl TripSpawner {
    pub fn new() -> TripSpawner {
        TripSpawner { trips: Vec::new() }
    }

    pub fn schedule_trip(
        &mut self,
        person: &Person,
        start_time: Time,
        mut spec: TripSpec,
        trip_start: TripEndpoint,
        cancelled: bool,
        modified: bool,
        map: &Map,
    ) {
        // TODO We'll want to repeat this validation when we spawn stuff later for a second leg...
        match &spec {
            TripSpec::VehicleAppearing {
                start_pos, goal, ..
            } => {
                if start_pos.dist_along() >= map.get_l(start_pos.lane()).length() {
                    panic!("Can't spawn at {}; it isn't that long", start_pos);
                }
                match goal {
                    DrivingGoal::Border(_, end_lane, _) => {
                        if start_pos.lane() == *end_lane
                            && start_pos.dist_along() == map.get_l(*end_lane).length()
                        {
                            panic!(
                                "Can't start at {}; it's the edge of a border already",
                                start_pos
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
                // TODO Might not be possible to walk to the same border if there's no sidewalk
                let backup_plan = match goal {
                    DrivingGoal::ParkNear(b) => Some(TripSpec::JustWalking {
                        start: SidewalkSpot::building(*start, map),
                        goal: SidewalkSpot::building(*b, map),
                    }),
                    DrivingGoal::Border(i, _, off_map) => {
                        SidewalkSpot::end_at_border(*i, off_map.clone(), map).map(|goal| {
                            TripSpec::JustWalking {
                                start: SidewalkSpot::building(*start, map),
                                goal,
                            }
                        })
                    }
                };

                if SidewalkSpot::bike_from_bike_rack(map.get_b(*start).sidewalk(), map).is_none() {
                    if backup_plan.is_some() {
                        println!(
                            "Can't start biking from {}; no biking or driving lane nearby? \
                             Walking instead",
                            start
                        );
                        spec = backup_plan.unwrap();
                    } else {
                        panic!(
                            "Can't start biking from {}; no biking or driving lane nearby? Can't \
                             walk instead, goal is {:?}",
                            start, goal
                        );
                    }
                } else if let DrivingGoal::ParkNear(b) = goal {
                    let last_lane = goal.goal_pos(PathConstraints::Bike, map).lane();
                    // If bike_to_sidewalk works, then SidewalkSpot::bike_rack should too.
                    if map
                        .get_parent(last_lane)
                        .bike_to_sidewalk(last_lane)
                        .is_none()
                    {
                        println!(
                            "Can't fulfill {:?} for a bike trip; no sidewalk near {}. Walking \
                             instead.",
                            goal, last_lane
                        );
                        spec = backup_plan.unwrap();
                    } else if map.get_b(*start).sidewalk() == map.get_b(*b).sidewalk() {
                        // A bike trip going from one lane to the same lane should... just walk.
                        println!(
                            "Bike trip from {} to {:?} will just walk; it's the same sidewalk!",
                            start, goal
                        );
                        spec = backup_plan.unwrap();
                    }
                }
            }
            TripSpec::UsingTransit { .. } => {}
            TripSpec::Remote { .. } => {}
        };

        self.trips
            .push((person.id, start_time, spec, trip_start, cancelled, modified));
    }

    pub fn finalize(
        mut self,
        map: &Map,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        timer: &mut Timer,
    ) {
        let pathfinding_upfront = trips.pathfinding_upfront;
        let profile = false;
        if profile {
            abstutil::start_profiler();
        }
        let paths = timer.parallelize(
            "calculate paths",
            Parallelism::Fastest,
            std::mem::replace(&mut self.trips, Vec::new()),
            |tuple| {
                let req = tuple.2.get_pathfinding_request(map);
                (
                    tuple,
                    req.clone(),
                    if pathfinding_upfront {
                        req.and_then(|r| map.pathfind(r))
                    } else {
                        None
                    },
                )
            },
        );
        if profile {
            abstutil::stop_profiler();
        }

        timer.start_iter("spawn trips", paths.len());
        for ((p, start_time, spec, trip_start, cancelled, modified), maybe_req, maybe_path) in paths
        {
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
                        modified,
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
                        modified,
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
                        modified,
                        legs,
                        map,
                    )
                }
                TripSpec::JustWalking { goal, .. } => trips.new_trip(
                    person.id,
                    start_time,
                    trip_start,
                    TripMode::Walk,
                    modified,
                    vec![TripLeg::Walk(goal.clone())],
                    map,
                ),
                TripSpec::UsingBike { bike, start, goal } => {
                    let walk_to =
                        SidewalkSpot::bike_from_bike_rack(map.get_b(start).sidewalk(), map)
                            .unwrap();
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
                    trips.new_trip(
                        person.id,
                        start_time,
                        trip_start,
                        TripMode::Bike,
                        modified,
                        legs,
                        map,
                    )
                }
                TripSpec::UsingTransit {
                    route,
                    stop1,
                    maybe_stop2,
                    goal,
                    ..
                } => {
                    let walk_to = SidewalkSpot::bus_stop(stop1, map);
                    let legs = if let Some(stop2) = maybe_stop2 {
                        vec![
                            TripLeg::Walk(walk_to.clone()),
                            TripLeg::RideBus(route, Some(stop2)),
                            TripLeg::Walk(goal),
                        ]
                    } else {
                        vec![
                            TripLeg::Walk(walk_to.clone()),
                            TripLeg::RideBus(route, None),
                        ]
                    };
                    trips.new_trip(
                        person.id,
                        start_time,
                        trip_start,
                        TripMode::Transit,
                        modified,
                        legs,
                        map,
                    )
                }
                TripSpec::Remote { to, mode, .. } => trips.new_trip(
                    person.id,
                    start_time,
                    trip_start,
                    mode,
                    modified,
                    vec![TripLeg::Remote(to)],
                    map,
                ),
            };

            if cancelled {
                trips.cancel_trip(trip);
            } else {
                scheduler.push(
                    start_time,
                    Command::StartTrip(trip, spec, maybe_req, maybe_path),
                );
            }
        }
    }
}

impl TripSpec {
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
                start: map.get_b(*start).sidewalk_pos,
                end: SidewalkSpot::bike_from_bike_rack(map.get_b(*start).sidewalk(), map)
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
