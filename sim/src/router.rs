use crate::mechanics::Queue;
use crate::{
    CarID, Event, ParkingSimState, ParkingSpot, PersonID, SidewalkSpot, TripID, TripPhaseType,
    Vehicle, VehicleType,
};
use geom::Distance;
use map_model::{
    BuildingID, IntersectionID, LaneID, Map, Path, PathConstraints, PathRequest, PathStep,
    Position, Traversable, TurnID,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Router {
    // Front is always the current step
    path: Path,
    goal: Goal,
    owner: CarID,
}

#[derive(Debug)]
pub enum ActionAtEnd {
    VanishAtBorder(IntersectionID),
    StartParking(ParkingSpot),
    GotoLaneEnd,
    StopBiking(SidewalkSpot),
    BusAtStop,
    GiveUpOnParking,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
enum Goal {
    // Spot and cached distance along the last driving lane
    // TODO Right now, the building is ignored when choosing the best spot.
    ParkNearBuilding {
        target: BuildingID,
        spot: Option<(ParkingSpot, Distance)>,
        // No parking available at all!
        stuck_end_dist: Option<Distance>,
        started_looking: bool,
    },
    EndAtBorder {
        end_dist: Distance,
        i: IntersectionID,
    },
    BikeThenStop {
        goal: SidewalkSpot,
    },
    FollowBusRoute {
        end_dist: Distance,
    },
}

impl Router {
    pub fn end_at_border(
        owner: CarID,
        path: Path,
        end_dist: Distance,
        i: IntersectionID,
    ) -> Router {
        Router {
            path,
            goal: Goal::EndAtBorder { end_dist, i },
            owner,
        }
    }
    pub fn vanish_bus(owner: CarID, l: LaneID, map: &Map) -> Router {
        let lane = map.get_l(l);
        Router {
            path: Path::one_step(l, map),
            goal: Goal::EndAtBorder {
                end_dist: lane.length(),
                i: lane.dst_i,
            },
            owner,
        }
    }

    pub fn park_near(owner: CarID, path: Path, bldg: BuildingID) -> Router {
        Router {
            path,
            goal: Goal::ParkNearBuilding {
                target: bldg,
                spot: None,
                stuck_end_dist: None,
                started_looking: false,
            },
            owner,
        }
    }

    pub fn bike_then_stop(owner: CarID, path: Path, goal: SidewalkSpot) -> Router {
        Router {
            goal: Goal::BikeThenStop { goal },
            path,
            owner,
        }
    }

    pub fn follow_bus_route(owner: CarID, path: Path, end_dist: Distance) -> Router {
        Router {
            path,
            goal: Goal::FollowBusRoute { end_dist },
            owner,
        }
    }

    pub fn head(&self) -> Traversable {
        self.path.current_step().as_traversable()
    }

    pub fn next(&self) -> Traversable {
        self.path.next_step().as_traversable()
    }

    pub fn maybe_next(&self) -> Option<Traversable> {
        if self.last_step() {
            None
        } else {
            Some(self.path.next_step().as_traversable())
        }
    }

    pub fn last_step(&self) -> bool {
        self.path.is_last_step()
    }

    pub fn get_end_dist(&self) -> Distance {
        // Shouldn't ask earlier!
        assert!(self.last_step());
        match self.goal {
            Goal::EndAtBorder { end_dist, .. } => end_dist,
            Goal::ParkNearBuilding {
                spot,
                stuck_end_dist,
                ..
            } => stuck_end_dist.unwrap_or_else(|| spot.unwrap().1),
            Goal::BikeThenStop { ref goal } => goal.sidewalk_pos.dist_along(),
            Goal::FollowBusRoute { end_dist } => end_dist,
        }
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }

    // Returns the step just finished
    pub fn advance(
        &mut self,
        vehicle: &Vehicle,
        parking: &ParkingSimState,
        map: &Map,
        trip_and_person: Option<(TripID, PersonID)>,
        events: &mut Vec<Event>,
    ) -> Traversable {
        let prev = self.path.shift(map).as_traversable();
        if self.last_step() {
            // Do this to trigger the side-effect of looking for parking.
            self.maybe_handle_end(
                Distance::ZERO,
                vehicle,
                parking,
                map,
                trip_and_person,
                events,
            );
        }

        // Sanity check laws haven't been broken
        if let Traversable::Lane(l) = self.head() {
            let lane = map.get_l(l);
            if !vehicle.vehicle_type.to_constraints().can_use(lane, map) {
                panic!(
                    "{} just wound up on {}, a {:?} (check the OSM tags)",
                    vehicle.id, l, lane.lane_type
                );
            }
        }

        prev
    }

    // Called when the car is Queued at the last step, or when they initially advance to the last
    // step.
    pub fn maybe_handle_end(
        &mut self,
        front: Distance,
        vehicle: &Vehicle,
        parking: &ParkingSimState,
        map: &Map,
        // TODO Not so nice to plumb all of this here
        trip_and_person: Option<(TripID, PersonID)>,
        events: &mut Vec<Event>,
    ) -> Option<ActionAtEnd> {
        match self.goal {
            Goal::EndAtBorder { end_dist, i } => {
                if end_dist == front {
                    Some(ActionAtEnd::VanishAtBorder(i))
                } else {
                    None
                }
            }
            Goal::ParkNearBuilding {
                ref mut spot,
                ref mut stuck_end_dist,
                target,
                ref mut started_looking,
            } => {
                if let Some(d) = stuck_end_dist {
                    if *d == front {
                        return Some(ActionAtEnd::GiveUpOnParking);
                    } else {
                        return None;
                    }
                }

                let need_new_spot = match spot {
                    Some((s, _)) => !parking.is_free(*s),
                    None => true,
                };
                if need_new_spot {
                    *started_looking = true;
                    let current_lane = self.path.current_step().as_lane();
                    let candidates = parking.get_all_free_spots(
                        Position::new(current_lane, front),
                        vehicle,
                        target,
                        map,
                    );
                    let best =
                        if let Some((driving_pos, _)) = map.get_b(target).driving_connection(map) {
                            if driving_pos.lane() == current_lane {
                                let target_dist = driving_pos.dist_along();
                                // Closest to the building
                                candidates
                                    .into_iter()
                                    .min_by_key(|(_, pos)| (pos.dist_along() - target_dist).abs())
                            } else {
                                // Closest to the road endpoint, I guess
                                candidates
                                    .into_iter()
                                    .min_by_key(|(_, pos)| pos.dist_along())
                            }
                        } else {
                            // Closest to the road endpoint, I guess
                            candidates
                                .into_iter()
                                .min_by_key(|(_, pos)| pos.dist_along())
                        };
                    if let Some((new_spot, new_pos)) = best {
                        if let Some((t, p)) = trip_and_person {
                            events.push(Event::TripPhaseStarting(
                                t,
                                p,
                                Some(PathRequest {
                                    start: Position::new(current_lane, front),
                                    end: new_pos,
                                    constraints: PathConstraints::Car,
                                }),
                                TripPhaseType::Parking,
                            ));
                        }
                        assert_eq!(new_pos.lane(), current_lane);
                        assert!(new_pos.dist_along() >= front);
                        *spot = Some((new_spot, new_pos.dist_along()));
                    } else {
                        if let Some((new_path_steps, new_spot, new_pos)) =
                            parking.path_to_free_parking_spot(current_lane, vehicle, target, map)
                        {
                            assert!(!new_path_steps.is_empty());
                            for step in new_path_steps {
                                self.path.add(step, map);
                            }
                            *spot = Some((new_spot, new_pos.dist_along()));
                            events.push(Event::PathAmended(self.path.clone()));
                            // TODO This path might not be the same as the one found here...
                            if let Some((t, p)) = trip_and_person {
                                events.push(Event::TripPhaseStarting(
                                    t,
                                    p,
                                    Some(PathRequest {
                                        start: Position::new(current_lane, front),
                                        end: new_pos,
                                        constraints: PathConstraints::Car,
                                    }),
                                    TripPhaseType::Parking,
                                ));
                            }
                        } else {
                            println!(
                                "WARNING: {} can't find parking on {} or anywhere reachable from \
                                 it. Possibly we're just totally out of parking space!",
                                vehicle.id, current_lane
                            );
                            *stuck_end_dist = Some(map.get_l(current_lane).length());
                        }
                        return Some(ActionAtEnd::GotoLaneEnd);
                    }
                }

                if spot.unwrap().1 == front {
                    Some(ActionAtEnd::StartParking(spot.unwrap().0))
                } else {
                    None
                }
            }
            Goal::BikeThenStop { ref goal } => {
                if goal.sidewalk_pos.dist_along() == front {
                    Some(ActionAtEnd::StopBiking(goal.clone()))
                } else {
                    None
                }
            }
            Goal::FollowBusRoute { end_dist } => {
                if end_dist == front {
                    Some(ActionAtEnd::BusAtStop)
                } else {
                    None
                }
            }
        }
    }

    pub fn opportunistically_lanechange(
        &mut self,
        queues: &BTreeMap<Traversable, Queue>,
        map: &Map,
    ) {
        if self.path.approaching_uber_turn() || self.path.currently_inside_ut().is_some() {
            return;
        }
        let (current_turn, next_lane) = {
            let steps = self.path.get_steps();
            if steps.len() < 5 {
                return;
            }
            match (steps[1], steps[4]) {
                (PathStep::Turn(t), PathStep::Lane(l)) => (t, l),
                _ => {
                    return;
                }
            }
        };

        let orig_target_lane = current_turn.dst;
        let parent = map.get_parent(orig_target_lane);
        let next_parent = map.get_l(next_lane).src_i;

        // Look for other candidates, and assign a cost to each.
        let constraints = self.owner.1.to_constraints();
        let (_, turn1, best_lane, turn2) = parent
            .children(parent.is_forwards(orig_target_lane))
            .iter()
            .filter(|(l, _)| constraints.can_use(map.get_l(*l), map))
            .filter_map(|(l, _)| {
                let t1 = TurnID {
                    parent: current_turn.parent,
                    src: current_turn.src,
                    dst: *l,
                };
                if let Some(turn1) = map.maybe_get_t(t1) {
                    // Make sure we can go from this lane to next_lane.
                    if let Some(turn2) = map.maybe_get_t(TurnID {
                        parent: next_parent,
                        src: *l,
                        dst: next_lane,
                    }) {
                        Some((turn1, *l, turn2))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .map(|(turn1, l, turn2)| {
                let (lt, lc, mut rightmost) = turn1.penalty(map);
                let (vehicles, mut bike) = queues[&Traversable::Lane(l)].target_lane_penalty();

                // The magic happens here. We have different penalties:
                //
                // 1) Are we headed towards a general purpose lane instead of a dedicated bike/bus
                //    lane?
                // 2) Are there any bikes in the target lane? This ONLY matters if we're a car. If
                //    we're another bike, the speed difference won't matter.
                // 3) IF we're a bike, are we headed to something other than the rightmost lane?
                // 4) Are there lots of vehicles stacked up in one lane?
                // 5) Are we changing lanes?
                //
                // A linear combination of these penalties is hard to reason about. Instead, we
                // make our choice based on each penalty in order, breaking ties by moving onto the
                // next thing.
                if self.owner.1 == VehicleType::Bike {
                    bike = 0;
                } else {
                    rightmost = 0;
                }
                let cost = (lt, bike, rightmost, vehicles, lc);

                (cost, turn1, l, turn2)
            })
            .min_by_key(|(cost, _, _, _)| *cost)
            .unwrap();
        // TODO Only switch if the target queue is some amount better; don't oscillate
        // unnecessarily.
        if best_lane == orig_target_lane {
            return;
        }

        self.path.modify_step(1, PathStep::Turn(turn1.id), map);
        self.path.modify_step(2, PathStep::Lane(best_lane), map);
        self.path.modify_step(3, PathStep::Turn(turn2.id), map);
    }

    pub fn replace_path_for_serialization(&mut self, path: Path) -> Path {
        std::mem::replace(&mut self.path, path)
    }

    pub fn is_parking(&self) -> bool {
        match self.goal {
            Goal::ParkNearBuilding {
                started_looking, ..
            } => started_looking,
            _ => false,
        }
    }
}
