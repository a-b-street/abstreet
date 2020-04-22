use crate::mechanics::Queue;
use crate::{
    Event, ParkingSimState, ParkingSpot, PersonID, SidewalkSpot, TripID, TripMode, TripPhaseType,
    Vehicle,
};
use geom::Distance;
use map_model::{
    BuildingID, IntersectionID, Map, Path, PathConstraints, PathRequest, PathStep, Position,
    Traversable, TurnID,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Router {
    // Front is always the current step
    path: Path,
    goal: Goal,
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
    },
    EndAtBorder {
        end_dist: Distance,
        i: IntersectionID,
    },
    BikeThenStop {
        end_dist: Distance,
    },
    FollowBusRoute {
        end_dist: Distance,
    },
}

impl Router {
    pub fn end_at_border(path: Path, end_dist: Distance, i: IntersectionID) -> Router {
        Router {
            path,
            goal: Goal::EndAtBorder { end_dist, i },
        }
    }

    pub fn park_near(path: Path, bldg: BuildingID) -> Router {
        Router {
            path,
            goal: Goal::ParkNearBuilding {
                target: bldg,
                spot: None,
                stuck_end_dist: None,
            },
        }
    }

    pub fn bike_then_stop(path: Path, end_dist: Distance, map: &Map) -> Option<Router> {
        let last_lane = path.get_steps().iter().last().unwrap().as_lane();
        if map
            .get_parent(last_lane)
            .bike_to_sidewalk(last_lane)
            .is_some()
        {
            Some(Router {
                path,
                goal: Goal::BikeThenStop { end_dist },
            })
        } else {
            println!("{} is the end of a bike route, with no sidewalk", last_lane);
            None
        }
    }

    pub fn follow_bus_route(path: Path, end_dist: Distance) -> Router {
        Router {
            path,
            goal: Goal::FollowBusRoute { end_dist },
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
            Goal::BikeThenStop { end_dist } => end_dist,
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
                ..
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
                    let current_lane = self.path.current_step().as_lane();
                    if let Some((new_spot, new_pos)) = parking.get_first_free_spot(
                        Position::new(current_lane, front),
                        vehicle,
                        map,
                    ) {
                        if let Some((t, p)) = trip_and_person {
                            events.push(Event::TripPhaseStarting(
                                t,
                                p,
                                TripMode::Drive,
                                Some(PathRequest {
                                    start: Position::new(current_lane, front),
                                    end: new_pos,
                                    constraints: PathConstraints::Car,
                                }),
                                TripPhaseType::Parking,
                            ));
                        }
                        *spot = Some((new_spot, new_pos.dist_along()));
                    } else {
                        if let Some((new_path_steps, new_spot, new_pos)) =
                            parking.path_to_free_parking_spot(current_lane, vehicle, map)
                        {
                            *spot = Some((new_spot, new_pos.dist_along()));
                            for step in new_path_steps {
                                self.path.add(step, map);
                            }
                            events.push(Event::PathAmended(self.path.clone()));
                            // TODO This path might not be the same as the one found here...
                            if let Some((t, p)) = trip_and_person {
                                events.push(Event::TripPhaseStarting(
                                    t,
                                    p,
                                    TripMode::Drive,
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
            Goal::BikeThenStop { end_dist } => {
                if end_dist == front {
                    // Checked up-front that this exists
                    let last_lane = self.head().as_lane();
                    let sidewalk = map
                        .get_parent(last_lane)
                        .bike_to_sidewalk(last_lane)
                        .unwrap();
                    Some(ActionAtEnd::StopBiking(
                        SidewalkSpot::bike_rack(sidewalk, map).unwrap(),
                    ))
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

        // Look for other candidate lanes. Must be the same lane type -- if there was a bus/bike
        // lane originally and pathfinding already decided to use it, stick with that decision.
        let orig_lt = map.get_l(orig_target_lane).lane_type;
        let siblings = if parent.is_forwards(orig_target_lane) {
            &parent.children_forwards
        } else {
            &parent.children_backwards
        };

        let (_, turn1, best_lane, turn2) = siblings
            .iter()
            .filter_map(|(l, lt)| {
                let turn1 = TurnID {
                    parent: current_turn.parent,
                    src: current_turn.src,
                    dst: *l,
                };
                if orig_lt == *lt && map.maybe_get_t(turn1).is_some() {
                    // Now make sure we can go from this lane to next_lane.
                    let turn2 = TurnID {
                        parent: next_parent,
                        src: *l,
                        dst: next_lane,
                    };
                    if map.maybe_get_t(turn2).is_some() {
                        Some((queues[&Traversable::Lane(*l)].cars.len(), turn1, *l, turn2))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .min_by_key(|(len, _, _, _)| *len)
            .unwrap();
        // TODO Only switch if the target queue is some amount better; don't oscillate
        // unnecessarily.
        // TODO Better weight function... any slower vehicles in one?
        if best_lane == orig_target_lane {
            return;
        }

        self.path.modify_step(1, PathStep::Turn(turn1), map);
        self.path.modify_step(2, PathStep::Lane(best_lane), map);
        self.path.modify_step(3, PathStep::Turn(turn2), map);
    }

    pub fn replace_path_for_serialization(&mut self, path: Path) -> Path {
        std::mem::replace(&mut self.path, path)
    }
}
