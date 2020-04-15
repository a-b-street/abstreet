use crate::{
    CarID, Command, CreateCar, CreatePedestrian, DrivingGoal, ParkingSimState, ParkingSpot,
    PedestrianID, PersonID, Scheduler, SidewalkPOI, SidewalkSpot, Sim, TripEndpoint, TripLeg,
    TripManager, VehicleSpec, VehicleType, MAX_CAR_LENGTH,
};
use abstutil::Timer;
use geom::{Speed, Time, EPSILON_DIST};
use map_model::{BuildingID, BusRouteID, BusStopID, Map, PathConstraints, PathRequest, Position};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TripSpec {
    // Can be used to spawn from a border or anywhere for interactive debugging.
    CarAppearing {
        start_pos: Position,
        goal: DrivingGoal,
        vehicle_spec: VehicleSpec,
        ped_speed: Speed,
    },
    UsingParkedCar {
        start: SidewalkSpot,
        spot: ParkingSpot,
        goal: DrivingGoal,
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
// TODO The API isn't great. Passing in Sim and having to use friend methods is awkward.
// Alternatives could be somehow consuming Sim temporarily and spitting it back out at the end
// (except the interactive spawner would have to mem::replace with a blank Sim?), or just queueing
// up commands and doing them at the end while holding onto &Sim.
pub struct TripSpawner {
    parked_cars_claimed: BTreeSet<CarID>,
    trips: Vec<(
        PersonID,
        Time,
        Option<PedestrianID>,
        Option<CarID>,
        TripSpec,
    )>,
}

impl TripSpawner {
    pub fn new() -> TripSpawner {
        TripSpawner {
            parked_cars_claimed: BTreeSet::new(),
            trips: Vec::new(),
        }
    }

    pub fn schedule_trip(
        &mut self,
        person: PersonID,
        start_time: Time,
        spec: TripSpec,
        map: &Map,
        sim: &mut Sim,
    ) -> (Option<PedestrianID>, Option<CarID>) {
        let (ped_id, car_id) = match spec {
            TripSpec::CarAppearing {
                ref vehicle_spec,
                ref goal,
                ..
            } => {
                let car = CarID(sim.spawner_new_car_id(), vehicle_spec.vehicle_type);
                let ped = match goal {
                    DrivingGoal::ParkNear(_) => {
                        let id = PedestrianID(sim.spawner_new_ped_id());
                        Some(id)
                    }
                    _ => None,
                };
                (ped, Some(car))
            }
            TripSpec::UsingParkedCar { .. }
            | TripSpec::MaybeUsingParkedCar { .. }
            | TripSpec::JustWalking { .. }
            | TripSpec::UsingTransit { .. } => {
                let id = PedestrianID(sim.spawner_new_ped_id());
                (Some(id), None)
            }
            TripSpec::UsingBike { .. } => {
                let ped = PedestrianID(sim.spawner_new_ped_id());
                let car = CarID(sim.spawner_new_car_id(), VehicleType::Bike);
                (Some(ped), Some(car))
            }
        };

        self.inner_schedule_trip(person, start_time, ped_id, car_id, spec, map, sim);

        (ped_id, car_id)
    }

    // TODO Maybe collapse this in the future
    fn inner_schedule_trip(
        &mut self,
        person: PersonID,
        start_time: Time,
        ped_id: Option<PedestrianID>,
        car_id: Option<CarID>,
        spec: TripSpec,
        map: &Map,
        sim: &Sim,
    ) {
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
                        "Can't spawn a car at {}; too close to the start",
                        start_pos.dist_along()
                    );
                }
                if start_pos.dist_along() >= map.get_l(start_pos.lane()).length() {
                    panic!(
                        "Can't spawn a car at {}; {} isn't that long",
                        start_pos.dist_along(),
                        start_pos.lane()
                    );
                }
                match goal {
                    DrivingGoal::Border(_, end_lane) => {
                        if start_pos.lane() == *end_lane
                            && start_pos.dist_along() == map.get_l(*end_lane).length()
                        {
                            panic!("Can't start a car at the edge of a border already");
                        }
                    }
                    DrivingGoal::ParkNear(_) => {}
                }
            }
            TripSpec::UsingParkedCar { spot, .. } => {
                let car_id = sim
                    .spawner_parking()
                    .get_car_at_spot(*spot)
                    .unwrap()
                    .vehicle
                    .id;
                if self.parked_cars_claimed.contains(&car_id) {
                    panic!(
                        "A TripSpec wants to use {}, which is already claimed",
                        car_id
                    );
                }
                self.parked_cars_claimed.insert(car_id);
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
                            ped_id,
                            None,
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

        self.trips.push((person, start_time, ped_id, car_id, spec));
    }

    pub fn finalize(
        mut self,
        map: &Map,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        parking: &ParkingSimState,
        timer: &mut Timer,
        retry_if_no_room: bool,
    ) {
        let paths = timer.parallelize(
            "calculate paths",
            std::mem::replace(&mut self.trips, Vec::new()),
            |tuple| {
                let req = tuple.4.get_pathfinding_request(map, parking);
                (tuple, req.clone(), map.pathfind(req))
            },
        );

        timer.start_iter("spawn trips", paths.len());
        for ((person, start_time, ped_id, car_id, spec), req, maybe_path) in paths {
            timer.next();
            match spec {
                TripSpec::CarAppearing {
                    start_pos,
                    vehicle_spec,
                    goal,
                    ped_speed,
                } => {
                    let vehicle = vehicle_spec.make(car_id.unwrap(), Some(person));
                    let mut legs = vec![TripLeg::Drive(vehicle.clone(), goal.clone())];
                    if let DrivingGoal::ParkNear(b) = goal {
                        legs.push(TripLeg::Walk(
                            ped_id.unwrap(),
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
                TripSpec::UsingParkedCar {
                    start,
                    spot,
                    goal,
                    ped_speed,
                } => {
                    let vehicle = &parking.get_car_at_spot(spot).unwrap().vehicle;
                    assert_eq!(vehicle.owner, Some(person));
                    let start_bldg = match start.connection {
                        SidewalkPOI::Building(b) => b,
                        _ => unreachable!(),
                    };

                    let parking_spot = SidewalkSpot::parking_spot(spot, map, parking);

                    let mut legs = vec![
                        TripLeg::Walk(ped_id.unwrap(), ped_speed, parking_spot.clone()),
                        TripLeg::Drive(vehicle.clone(), goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(
                                ped_id.unwrap(),
                                ped_speed,
                                SidewalkSpot::building(b, map),
                            ));
                        }
                        DrivingGoal::Border(_, _) => {}
                    }
                    let trip =
                        trips.new_trip(person, start_time, TripEndpoint::Bldg(start_bldg), legs);

                    if let Some(path) = maybe_path {
                        scheduler.push(
                            start_time,
                            Command::SpawnPed(CreatePedestrian {
                                id: ped_id.unwrap(),
                                speed: ped_speed,
                                start,
                                goal: parking_spot,
                                path,
                                req,
                                trip,
                                person,
                            }),
                        );
                    } else {
                        timer.warn(format!(
                            "UsingParkedCar trip couldn't find the first path {}",
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
                    let legs = vec![TripLeg::Walk(ped_id.unwrap(), ped_speed, walk_to.clone())];
                    let trip =
                        trips.new_trip(person, start_time, TripEndpoint::Bldg(start_bldg), legs);

                    scheduler.push(
                        start_time,
                        Command::SpawnPed(CreatePedestrian {
                            id: ped_id.unwrap(),
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
                        vec![TripLeg::Walk(ped_id.unwrap(), ped_speed, goal.clone())],
                    );

                    if let Some(path) = maybe_path {
                        scheduler.push(
                            start_time,
                            Command::SpawnPed(CreatePedestrian {
                                id: ped_id.unwrap(),
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
                    let walk_to =
                        SidewalkSpot::bike_from_bike_rack(start.sidewalk_pos.lane(), map).unwrap();
                    let mut legs = vec![
                        TripLeg::Walk(ped_id.unwrap(), ped_speed, walk_to.clone()),
                        TripLeg::Drive(vehicle.make(car_id.unwrap(), None), goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(
                                ped_id.unwrap(),
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
                                id: ped_id.unwrap(),
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
                            TripLeg::Walk(ped_id.unwrap(), ped_speed, walk_to.clone()),
                            TripLeg::RideBus(ped_id.unwrap(), route, stop2),
                            TripLeg::Walk(ped_id.unwrap(), ped_speed, goal),
                        ],
                    );

                    if let Some(path) = maybe_path {
                        scheduler.push(
                            start_time,
                            Command::SpawnPed(CreatePedestrian {
                                id: ped_id.unwrap(),
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
    pub fn spawn_car_at(pos: Position, map: &Map) -> Option<Position> {
        let len = map.get_l(pos.lane()).length();
        // There's no hope.
        if len <= MAX_CAR_LENGTH {
            return None;
        }

        if pos.dist_along() < MAX_CAR_LENGTH {
            Some(Position::new(pos.lane(), MAX_CAR_LENGTH))
        } else if pos.dist_along() == len {
            Some(Position::new(pos.lane(), pos.dist_along() - EPSILON_DIST))
        } else {
            Some(pos)
        }
    }

    pub(crate) fn get_pathfinding_request(
        &self,
        map: &Map,
        parking: &ParkingSimState,
    ) -> PathRequest {
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
            TripSpec::UsingParkedCar { start, spot, .. } => PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::parking_spot(*spot, map, parking).sidewalk_pos,
                constraints: PathConstraints::Pedestrian,
            },
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
