use crate::plugins::sim::new_des_model::{ParkingSimState, ParkingSpot, Vehicle};
use geom::Distance;
use map_model::{BuildingID, LaneType, Map, Position, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Router {
    // Front is always the current step
    path: VecDeque<Traversable>,
    goal: Goal,
}

pub enum ActionAtEnd {
    Vanish,
    StartParking(ParkingSpot),
    GotoLaneEnd,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
enum Goal {
    // Spot and distance along the last driving lane
    // TODO Right now, the building is ignored.
    ParkNearBuilding {
        target: BuildingID,
        spot: Option<(ParkingSpot, Distance)>,
    },
    // Stop at this distance along the last lane in the path
    StopSuddenly {
        end_dist: Distance,
    },
}

impl Router {
    pub fn stop_suddenly(path: Vec<Traversable>, end_dist: Distance, map: &Map) -> Router {
        if end_dist >= path.last().unwrap().length(map) {
            panic!(
                "Can't end a car at {}; {:?} isn't that long",
                end_dist,
                path.last().unwrap()
            );
        }

        Router {
            path: VecDeque::from(path),
            goal: Goal::StopSuddenly { end_dist },
        }
    }

    pub fn park_near(path: Vec<Traversable>, bldg: BuildingID) -> Router {
        Router {
            path: VecDeque::from(path),
            goal: Goal::ParkNearBuilding {
                target: bldg,
                spot: None,
            },
        }
    }

    pub fn validate_start_dist(&self, start_dist: Distance) {
        match self.goal {
            Goal::StopSuddenly { end_dist } => {
                if self.path.len() == 1 && start_dist >= end_dist {
                    panic!(
                        "Can't start a car with one path in its step and go from {} to {}",
                        start_dist, end_dist
                    );
                }
            }
            Goal::ParkNearBuilding { .. } => {}
        }
    }

    pub fn head(&self) -> Traversable {
        self.path[0]
    }

    pub fn next(&self) -> Traversable {
        self.path[1]
    }

    pub fn last_step(&self) -> bool {
        self.path.len() == 1
    }

    pub fn get_end_dist(&self) -> Distance {
        // Shouldn't ask earlier!
        assert!(self.last_step());
        match self.goal {
            Goal::StopSuddenly { end_dist } => end_dist,
            Goal::ParkNearBuilding { spot, .. } => spot.unwrap().1,
        }
    }

    // Returns the step just finished
    pub fn advance(
        &mut self,
        vehicle: &Vehicle,
        parking: &ParkingSimState,
        map: &Map,
    ) -> Traversable {
        let prev = self.path.pop_front().unwrap();
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
            Goal::StopSuddenly { end_dist } => {
                if end_dist == front {
                    Some(ActionAtEnd::Vanish)
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
                    if let Some(new_spot) = find_parking_spot(
                        Position::new(self.path[0].as_lane(), front),
                        vehicle,
                        map,
                        parking,
                    ) {
                        *spot = Some(new_spot);
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
        }
    }

    fn roam_around_for_parking(&mut self, vehicle: &Vehicle, map: &Map) {
        let choices = map.get_turns_from_lane(self.head().as_lane());
        if choices.is_empty() {
            // TODO Fix properly by picking and pathfinding fully to a nearby parking lane.
            println!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever. Vanishing.", vehicle.id, self.head().as_lane());
            self.goal = Goal::StopSuddenly {
                end_dist: self.head().length(map),
            };
            return;
        }
        // TODO Better strategies than this: look for lanes with free spots (if it'd be feasible to
        // physically see the spots), stay close to the original goal building, avoid lanes we've
        // visited, prefer easier turns...
        let turn = choices[0];
        self.path.push_back(Traversable::Turn(turn.id));
        self.path.push_back(Traversable::Lane(turn.id.dst));
    }
}

// Returns the spot and the driving distance along it for the vehicle to line up to.
fn find_parking_spot(
    driving_pos: Position,
    vehicle: &Vehicle,
    map: &Map,
    parking: &ParkingSimState,
) -> Option<(ParkingSpot, Distance)> {
    let parking_lane = map
        .find_closest_lane(driving_pos.lane(), vec![LaneType::Parking])
        .ok()?;
    let spot = parking.get_first_free_spot(driving_pos.equiv_pos(parking_lane, map), vehicle)?;
    Some((
        spot,
        parking
            .spot_to_driving_pos(spot, vehicle, driving_pos.lane(), map)
            .dist_along(),
    ))
}
