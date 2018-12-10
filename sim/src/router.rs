use crate::driving::Action;
use crate::kinematics;
use crate::kinematics::Vehicle;
use crate::parking::ParkingSimState;
use crate::transit::TransitSimState;
use crate::view::AgentView;
use crate::{Distance, Event, ParkingSpot, Tick};
use dimensioned::si;
use geom::EPSILON_DIST;
use map_model::{
    BuildingID, LaneID, LaneType, Map, Path, PathStep, Position, Trace, Traversable, TurnID,
};
use rand::{Rng, XorShiftRng};
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
enum Goal {
    ParkNearBuilding(BuildingID),
    // Stop at this distance along the last lane in the path
    BikeThenStop(Distance),
    FollowBusRoute,
    EndAtBorder,
}

// Gives higher-level instructions to a car.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Router {
    // The head of the path is the current lane or turn.
    path: Path,
    goal: Goal,
}

impl Router {
    pub fn make_router_to_park(path: Path, goal: BuildingID) -> Router {
        Router {
            path,
            goal: Goal::ParkNearBuilding(goal),
        }
    }

    pub fn make_bike_router(path: Path, dist: Distance) -> Router {
        Router {
            path,
            goal: Goal::BikeThenStop(dist),
        }
    }

    pub fn make_router_for_bus(first_path: Path) -> Router {
        Router {
            path: first_path,
            goal: Goal::FollowBusRoute,
        }
    }

    pub fn make_router_to_border(path: Path) -> Router {
        Router {
            path,
            goal: Goal::EndAtBorder,
        }
    }

    pub fn tooltip_line(&self) -> String {
        format!("{} lanes left in path", self.path.num_lanes())
    }

    // Mutable so we can roam around and try another road to park if the last one is unavailable.
    // It's safe to mutate in a react() phase, because we're observing a fixed state of the world
    // and augmenting the plan, but not the car's actual state.
    pub fn react_before_lookahead(
        &mut self,
        events: &mut Vec<Event>,
        view: &AgentView,
        vehicle: &Vehicle,
        time: Tick,
        map: &Map,
        parking_sim: &ParkingSimState,
        // Mutable so we can indicate state transitions
        transit_sim: &mut TransitSimState,
        rng: &mut XorShiftRng,
    ) -> Option<Action> {
        if self.path.isnt_last_step() || view.speed > kinematics::EPSILON_SPEED {
            return None;
        }

        match self.goal {
            Goal::ParkNearBuilding(_) => {
                let last_lane = view.on.as_lane();
                if let Some((spot, needed_driving_pos)) = find_parking_spot(
                    Position::new(last_lane, view.dist_along),
                    vehicle,
                    map,
                    parking_sim,
                ) {
                    if needed_driving_pos.dist_along() == view.dist_along {
                        return Some(Action::StartParking(spot));
                    }
                // Being stopped before the parking spot is normal if the final road is
                // clogged with other drivers.
                } else {
                    return self.look_for_parking(last_lane, view, map, rng);
                }
            }
            Goal::BikeThenStop(dist) => {
                // Do an epsilon check here, to avoid a bug observed before (and because all
                // distance checks really ought to be epsilon checks...)
                if view.dist_along - dist < EPSILON_DIST {
                    return Some(Action::StartParkingBike);
                }
            }
            Goal::FollowBusRoute => {
                let (should_idle, new_path) =
                    transit_sim.get_action_when_stopped_at_end(events, view, time, map);
                if let Some(p) = new_path {
                    self.path = p;
                }
                if should_idle {
                    return Some(Action::Continue(0.0 * si::MPS2, Vec::new()));
                }
            }
            // Don't stop at the border node; plow through
            Goal::EndAtBorder => {}
        }
        None
    }

    // If we return None, then the caller will immediately ask what turn to do.
    pub fn stop_early_at_dist(
        &self,
        // TODO urgh, we cant reuse AgentView here, because lookahead doesn't advance the view :(
        on: Traversable,
        dist_along: Distance,
        vehicle: &Vehicle,
        map: &Map,
        parking_sim: &ParkingSimState,
        transit_sim: &TransitSimState,
    ) -> Option<Distance> {
        if self.path.is_last_step() {
            match self.goal {
                Goal::ParkNearBuilding(_) => {
                    if let Some((_, needed_driving_pos)) = find_parking_spot(
                        Position::new(on.as_lane(), dist_along),
                        vehicle,
                        map,
                        parking_sim,
                    ) {
                        return Some(needed_driving_pos.dist_along());
                    } else {
                        // If lookahead runs out of path, then just stop at the end of that lane,
                        // and then reroute when react_before_lookahead is called later.
                        return Some(on.length(map));
                    }
                }
                Goal::BikeThenStop(dist) => {
                    return Some(dist);
                }
                Goal::FollowBusRoute => {
                    return Some(transit_sim.get_dist_to_stop_at(vehicle.id, on.as_lane()));
                }
                // The car shouldn't stop early!
                Goal::EndAtBorder => {}
            }
        }
        None
    }

    // Returns the next step
    pub fn finished_step(&mut self, on: Traversable) -> PathStep {
        let expected = match on {
            Traversable::Lane(id) => PathStep::Lane(id),
            Traversable::Turn(id) => PathStep::Turn(id),
        };
        assert_eq!(expected, self.path.shift());
        self.path.current_step()
    }

    // Called when lookahead reaches an intersection
    pub fn should_vanish_at_border(&self) -> bool {
        self.path.is_last_step() && self.goal == Goal::EndAtBorder
    }

    pub fn next_step_as_turn(&self) -> Option<TurnID> {
        if self.path.is_last_step() {
            return None;
        }
        if let PathStep::Turn(id) = self.path.next_step() {
            return Some(id);
        }
        None
    }

    fn look_for_parking(
        &mut self,
        last_lane: LaneID,
        view: &AgentView,
        map: &Map,
        rng: &mut XorShiftRng,
    ) -> Option<Action> {
        // TODO Better strategies than random: look for lanes with free spots (if it'd be feasible
        // to physically see the spots), stay close to the original goal building, avoid lanes
        // we've visited, prefer easier turns...
        let choices = map.get_next_turns_and_lanes(last_lane, map.get_l(last_lane).dst_i);
        if choices.is_empty() {
            // TODO Fix properly by picking and pathfinding fully to a nearby parking lane.
            error!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever. Vanishing.", view.id, last_lane);
            return Some(Action::TmpVanish);
        }
        let (turn, new_lane) = rng.choose(&choices).unwrap();
        if view.debug {
            debug!(
                "{} can't find parking on {}, so wandering over to {}",
                view.id, last_lane, new_lane.id
            );
        }
        self.path.add(PathStep::Turn(turn.id));
        self.path.add(PathStep::Lane(new_lane.id));
        None
    }

    pub fn trace_route(
        &self,
        start_dist: Distance,
        map: &Map,
        dist_ahead: Distance,
    ) -> Option<Trace> {
        self.path.trace(map, start_dist, dist_ahead)
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }
}

// Returns the spot and the driving position aligned to it, given an input position.
fn find_parking_spot(
    driving_pos: Position,
    vehicle: &Vehicle,
    map: &Map,
    parking_sim: &ParkingSimState,
) -> Option<(ParkingSpot, Position)> {
    let parking_lane = map
        .find_closest_lane(driving_pos.lane(), vec![LaneType::Parking])
        .ok()?;
    let spot = parking_sim.get_first_free_spot(driving_pos.equiv_pos(parking_lane, map))?;
    Some((
        spot,
        parking_sim.spot_to_driving_pos(spot, vehicle, driving_pos.lane(), map),
    ))
}
