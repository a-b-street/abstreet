//! Intermediate structures used to instantiate a Scenario. Badly needs simplification:
//! https://github.com/dabreegster/abstreet/issues/258

use rand::seq::SliceRandom;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};

use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathConstraints, PathRequest, Position,
};

use crate::{
    CarID, DrivingGoal, PersonID, SidewalkSpot, TripInfo, TripLeg, TripMode, VehicleType,
    SPAWN_DIST,
};

// TODO Some of these fields are unused now that we separately pass TripEndpoint
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub(crate) enum TripSpec {
    /// Can be used to spawn from a border or anywhere for interactive debugging.
    VehicleAppearing {
        start_pos: Position,
        goal: DrivingGoal,
        /// This must be a currently off-map vehicle owned by the person.
        use_vehicle: CarID,
        retry_if_no_room: bool,
    },
    /// Something went wrong spawning the trip.
    SpawningFailure {
        use_vehicle: Option<CarID>,
        error: String,
    },
    UsingParkedCar {
        /// This must be a currently parked vehicle owned by the person.
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
}

impl TripSpec {
    pub fn to_plan(
        self,
        person: PersonID,
        info: TripInfo,
        map: &Map,
    ) -> (PersonID, TripInfo, TripSpec, Vec<TripLeg>) {
        // TODO We'll want to repeat this validation when we spawn stuff later for a second leg...
        let mut legs = Vec::new();
        match &self {
            TripSpec::VehicleAppearing {
                start_pos,
                goal,
                use_vehicle,
                ..
            } => {
                if start_pos.dist_along() >= map.get_l(start_pos.lane()).length() {
                    panic!("Can't spawn at {}; it isn't that long", start_pos);
                }
                if let DrivingGoal::Border(_, end_lane) = goal {
                    if start_pos.lane() == *end_lane
                        && start_pos.dist_along() == map.get_l(*end_lane).length()
                    {
                        panic!(
                            "Can't start at {}; it's the edge of a border already",
                            start_pos
                        );
                    }
                }

                let constraints = if use_vehicle.1 == VehicleType::Bike {
                    PathConstraints::Bike
                } else {
                    PathConstraints::Car
                };

                legs.push(TripLeg::Drive(*use_vehicle, goal.clone()));
                if let DrivingGoal::ParkNear(b) = goal {
                    legs.push(TripLeg::Walk(SidewalkSpot::building(*b, map)));
                }

                if goal.goal_pos(constraints, map).is_none() {
                    return TripSpec::SpawningFailure {
                        use_vehicle: Some(use_vehicle.clone()),
                        error: format!("goal_pos to {:?} for a {:?} failed", goal, constraints),
                    }
                    .to_plan(person, info, map);
                }
            }
            TripSpec::SpawningFailure { .. } => {
                // TODO The legs are a lie. Since the trip gets cancelled, this doesn't matter.
                // I'm not going to bother doing better because I think TripLeg will get
                // revamped soon anyway.
                legs.push(TripLeg::RideBus(BusRouteID(0), None));
            }
            TripSpec::UsingParkedCar { car, goal, .. } => {
                legs.push(TripLeg::Walk(SidewalkSpot::deferred_parking_spot()));
                legs.push(TripLeg::Drive(*car, goal.clone()));
                match goal {
                    DrivingGoal::ParkNear(b) => {
                        legs.push(TripLeg::Walk(SidewalkSpot::building(*b, map)));
                    }
                    DrivingGoal::Border(_, _) => {}
                }
            }
            TripSpec::JustWalking { start, goal, .. } => {
                if start == goal {
                    panic!(
                        "A trip just walking from {:?} to {:?} doesn't make sense",
                        start, goal
                    );
                }
                legs.push(TripLeg::Walk(goal.clone()));
            }
            TripSpec::UsingBike { start, goal, bike } => {
                // TODO Might not be possible to walk to the same border if there's no sidewalk
                let backup_plan = match goal {
                    DrivingGoal::ParkNear(b) => Some(TripSpec::JustWalking {
                        start: SidewalkSpot::building(*start, map),
                        goal: SidewalkSpot::building(*b, map),
                    }),
                    DrivingGoal::Border(i, _) => {
                        SidewalkSpot::end_at_border(*i, map).map(|goal| TripSpec::JustWalking {
                            start: SidewalkSpot::building(*start, map),
                            goal,
                        })
                    }
                };

                if let Some(start_spot) = SidewalkSpot::bike_rack(*start, map) {
                    if let DrivingGoal::ParkNear(b) = goal {
                        if let Some(goal_spot) = SidewalkSpot::bike_rack(*b, map) {
                            if start_spot.sidewalk_pos.lane() == goal_spot.sidewalk_pos.lane() {
                                info!(
                                    "Bike trip from {} to {} will just walk; it's the same \
                                     sidewalk!",
                                    start, b
                                );
                                return backup_plan.unwrap().to_plan(person, info, map);
                            }
                        } else {
                            info!(
                                "Can't find biking connection for goal {}, walking instead",
                                b
                            );
                            return backup_plan.unwrap().to_plan(person, info, map);
                        }
                    }

                    legs.push(TripLeg::Walk(start_spot));
                    legs.push(TripLeg::Drive(*bike, goal.clone()));
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(SidewalkSpot::building(*b, map)));
                        }
                        DrivingGoal::Border(_, _) => {}
                    }
                } else if backup_plan.is_some() {
                    info!("Can't start biking from {}. Walking instead", start);
                    return backup_plan.unwrap().to_plan(person, info, map);
                } else {
                    return TripSpec::SpawningFailure {
                        use_vehicle: Some(*bike),
                        error: format!(
                            "Can't start biking from {} and can't walk either! Goal is {:?}",
                            start, goal
                        ),
                    }
                    .to_plan(person, info, map);
                }
            }
            TripSpec::UsingTransit {
                route,
                stop1,
                maybe_stop2,
                goal,
                ..
            } => {
                let walk_to = SidewalkSpot::bus_stop(*stop1, map);
                if let Some(stop2) = maybe_stop2 {
                    legs = vec![
                        TripLeg::Walk(walk_to.clone()),
                        TripLeg::RideBus(*route, Some(*stop2)),
                        TripLeg::Walk(goal.clone()),
                    ];
                } else {
                    legs = vec![
                        TripLeg::Walk(walk_to.clone()),
                        TripLeg::RideBus(*route, None),
                    ];
                }
            }
        };

        (person, info, self, legs)
    }

    /// Turn an origin/destination pair and mode into a specific plan for instantiating a trip.
    /// Decisions like how to use public transit happen here.
    pub fn maybe_new(
        from: TripEndpoint,
        to: TripEndpoint,
        mode: TripMode,
        use_vehicle: Option<CarID>,
        retry_if_no_room: bool,
        rng: &mut XorShiftRng,
        map: &Map,
    ) -> Result<TripSpec, String> {
        Ok(match mode {
            TripMode::Drive | TripMode::Bike => {
                let constraints = if mode == TripMode::Drive {
                    PathConstraints::Car
                } else {
                    PathConstraints::Bike
                };
                let goal = to.driving_goal(constraints, map)?;
                match from {
                    TripEndpoint::Bldg(start_bldg) => {
                        if mode == TripMode::Drive {
                            TripSpec::UsingParkedCar {
                                start_bldg,
                                goal,
                                car: use_vehicle.unwrap(),
                            }
                        } else {
                            TripSpec::UsingBike {
                                start: start_bldg,
                                goal,
                                bike: use_vehicle.unwrap(),
                            }
                        }
                    }
                    TripEndpoint::Border(i) => {
                        let start_lane = map
                            .get_i(i)
                            .some_outgoing_road(map)
                            .and_then(|dr| dr.lanes(constraints, map).choose(rng).cloned())
                            .ok_or_else(|| {
                                format!("can't start a {} trip from {}", mode.ongoing_verb(), i)
                            })?;
                        TripSpec::VehicleAppearing {
                            start_pos: Position::new(start_lane, SPAWN_DIST),
                            goal,
                            use_vehicle: use_vehicle.unwrap(),
                            retry_if_no_room,
                        }
                    }
                    TripEndpoint::SuddenlyAppear(start_pos) => TripSpec::VehicleAppearing {
                        start_pos,
                        goal,
                        use_vehicle: use_vehicle.unwrap(),
                        retry_if_no_room,
                    },
                }
            }
            TripMode::Walk => TripSpec::JustWalking {
                start: from.start_sidewalk_spot(map)?,
                goal: to.end_sidewalk_spot(map)?,
            },
            TripMode::Transit => {
                let start = from.start_sidewalk_spot(map)?;
                let goal = to.end_sidewalk_spot(map)?;
                if let Some((stop1, maybe_stop2, route)) =
                    map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
                {
                    TripSpec::UsingTransit {
                        start,
                        goal,
                        route,
                        stop1,
                        maybe_stop2,
                    }
                } else {
                    //timer.warn(format!("{:?} not actually using transit, because pathfinding
                    // didn't find any useful route", trip));
                    TripSpec::JustWalking { start, goal }
                }
            }
        })
    }
}

/// Specifies where a trip begins or ends.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum TripEndpoint {
    Bldg(BuildingID),
    Border(IntersectionID),
    /// Used for interactive spawning, tests, etc. For now, only valid as a trip's start.
    SuddenlyAppear(Position),
}

impl TripEndpoint {
    /// Figure out a single PathRequest that goes between two TripEndpoints. Assume a single mode
    /// the entire time -- no walking to a car before driving, for instance. The result probably
    /// won't be exactly what would happen on a real trip between the endpoints because of this
    /// assumption.
    pub fn path_req(
        from: TripEndpoint,
        to: TripEndpoint,
        mode: TripMode,
        map: &Map,
    ) -> Option<PathRequest> {
        Some(PathRequest {
            start: from.clone().pos(mode, true, map)?,
            end: to.clone().pos(mode, false, map)?,
            constraints: match mode {
                TripMode::Walk | TripMode::Transit => PathConstraints::Pedestrian,
                TripMode::Drive => PathConstraints::Car,
                TripMode::Bike => PathConstraints::Bike,
            },
        })
    }

    fn start_sidewalk_spot(&self, map: &Map) -> Result<SidewalkSpot, String> {
        match self {
            TripEndpoint::Bldg(b) => Ok(SidewalkSpot::building(*b, map)),
            TripEndpoint::Border(i) => SidewalkSpot::start_at_border(*i, map)
                .ok_or_else(|| format!("can't start walking from {}", i)),
            TripEndpoint::SuddenlyAppear(pos) => Ok(SidewalkSpot::suddenly_appear(*pos, map)),
        }
    }

    fn end_sidewalk_spot(&self, map: &Map) -> Result<SidewalkSpot, String> {
        match self {
            TripEndpoint::Bldg(b) => Ok(SidewalkSpot::building(*b, map)),
            TripEndpoint::Border(i) => SidewalkSpot::end_at_border(*i, map)
                .ok_or_else(|| format!("can't end walking at {}", i)),
            TripEndpoint::SuddenlyAppear(_) => unreachable!(),
        }
    }

    fn driving_goal(&self, constraints: PathConstraints, map: &Map) -> Result<DrivingGoal, String> {
        match self {
            TripEndpoint::Bldg(b) => Ok(DrivingGoal::ParkNear(*b)),
            TripEndpoint::Border(i) => map
                .get_i(*i)
                .some_incoming_road(map)
                .and_then(|dr| {
                    let lanes = dr.lanes(constraints, map);
                    if lanes.is_empty() {
                        None
                    } else {
                        // TODO ideally could use any
                        Some(DrivingGoal::Border(dr.dst_i(map), lanes[0]))
                    }
                })
                .ok_or_else(|| format!("can't end at {} for {:?}", i, constraints)),
            TripEndpoint::SuddenlyAppear(_) => unreachable!(),
        }
    }

    fn pos(self, mode: TripMode, from: bool, map: &Map) -> Option<Position> {
        match mode {
            TripMode::Walk | TripMode::Transit => (if from {
                self.start_sidewalk_spot(map)
            } else {
                self.end_sidewalk_spot(map)
            })
            .ok()
            .map(|spot| spot.sidewalk_pos),
            TripMode::Drive | TripMode::Bike => {
                if from {
                    match self {
                        // Fall through and use DrivingGoal also to start.
                        TripEndpoint::Bldg(_) => {}
                        TripEndpoint::Border(i) => {
                            return map.get_i(i).some_outgoing_road(map).and_then(|dr| {
                                dr.lanes(mode.to_constraints(), map)
                                    .get(0)
                                    .map(|l| Position::start(*l))
                            });
                        }
                        TripEndpoint::SuddenlyAppear(pos) => {
                            return Some(pos);
                        }
                    }
                }
                self.driving_goal(mode.to_constraints(), map)
                    .ok()
                    .and_then(|goal| goal.goal_pos(mode.to_constraints(), map))
            }
        }
    }
}
