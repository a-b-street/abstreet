use crate::{ParkingSimState, ParkingSpot, SidewalkSpot, Vehicle};
use geom::Distance;
use map_model::{BuildingID, IntersectionID, Map, Path, PathStep, Position, Traversable};
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Router {
    // Front is always the current step
    path: Path,
    goal: Goal,
}

pub enum ActionAtEnd {
    VanishAtBorder(IntersectionID),
    StartParking(ParkingSpot),
    GotoLaneEnd,
    StopBiking(SidewalkSpot),
    BusAtStop,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
enum Goal {
    // Spot and cached distance along the last driving lane
    // TODO Right now, the building is ignored.
    ParkNearBuilding {
        target: BuildingID,
        spot: Option<(ParkingSpot, Distance)>,
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
            },
        }
    }

    pub fn bike_then_stop(path: Path, end_dist: Distance) -> Router {
        Router {
            path,
            goal: Goal::BikeThenStop { end_dist },
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

    pub fn last_step(&self) -> bool {
        self.path.is_last_step()
    }

    pub fn get_end_dist(&self) -> Distance {
        // Shouldn't ask earlier!
        assert!(self.last_step());
        match self.goal {
            Goal::EndAtBorder { end_dist, .. } => end_dist,
            Goal::ParkNearBuilding { spot, .. } => spot.unwrap().1,
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
    ) -> Traversable {
        let prev = self.path.shift().as_traversable();
        if self.last_step() {
            // Do this to trigger the side-effect of looking for parking.
            self.maybe_handle_end(Distance::ZERO, vehicle, parking, map);
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
    ) -> Option<ActionAtEnd> {
        match self.goal {
            Goal::EndAtBorder { end_dist, i } => {
                if end_dist == front {
                    Some(ActionAtEnd::VanishAtBorder(i))
                } else {
                    None
                }
            }
            Goal::ParkNearBuilding { ref mut spot, .. } => {
                let need_new_spot = match spot {
                    Some((s, _)) => !parking.is_free(*s),
                    None => true,
                };
                if need_new_spot {
                    if let Some((new_spot, new_pos)) = parking.get_first_free_spot(
                        Position::new(self.path.current_step().as_traversable().as_lane(), front),
                        vehicle,
                        map,
                    ) {
                        *spot = Some((new_spot, new_pos.dist_along()));
                    } else {
                        self.roam_around_for_parking(vehicle, map);
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
                    let last_lane = self.head().as_lane();
                    Some(ActionAtEnd::StopBiking(
                        SidewalkSpot::bike_rack(
                            map.get_parent(last_lane)
                                .bike_to_sidewalk(last_lane)
                                .unwrap(),
                            map,
                        )
                        .unwrap(),
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

    fn roam_around_for_parking(&mut self, vehicle: &Vehicle, map: &Map) {
        let choices = map.get_turns_from_lane(self.head().as_lane());
        if choices.is_empty() {
            // TODO Fix properly by picking and pathfinding fully to a nearby parking lane.
            println!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever. Vanishing.", vehicle.id, self.head().as_lane());
            self.goal = Goal::EndAtBorder {
                end_dist: self.head().length(map),
                i: map.get_l(self.head().as_lane()).dst_i,
            };
            return;
        }
        // TODO Better strategies than this: look for lanes with free spots (if it'd be feasible to
        // physically see the spots), stay close to the original goal building, avoid lanes we've
        // visited, prefer easier turns...
        let turn = choices[0];
        self.path.add(PathStep::Turn(turn.id));
        self.path.add(PathStep::Lane(turn.id.dst));
    }
}
