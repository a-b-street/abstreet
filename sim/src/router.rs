use dimensioned::si;
use driving::Action;
use kinematics;
use kinematics::Vehicle;
use map_model::{BuildingID, LaneID, Map, Path, PathStep, Trace, Traversable, TurnID};
use parking::ParkingSimState;
use rand::{Rng, XorShiftRng};
use transit::TransitSimState;
use view::AgentView;
use {Distance, Event, ParkingSpot, Tick};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Goal {
    ParkNearBuilding(BuildingID),
    FollowBusRoute,
}

// Gives higher-level instructions to a car.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn make_router_for_bus(first_path: Path) -> Router {
        Router {
            path: first_path,
            goal: Goal::FollowBusRoute,
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
                if let Some(spot) = find_parking_spot(last_lane, view.dist_along, map, parking_sim)
                {
                    if parking_sim.dist_along_for_car(spot, vehicle) == view.dist_along {
                        return Some(Action::StartParking(spot));
                    }
                // Being stopped before the parking spot is normal if the final road is
                // clogged with other drivers.
                } else {
                    return self.look_for_parking(last_lane, view, map, rng);
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
                    if let Some(spot) =
                        find_parking_spot(on.as_lane(), dist_along, map, parking_sim)
                    {
                        return Some(parking_sim.dist_along_for_car(spot, vehicle));
                    } else {
                        // If lookahead runs out of path, then just stop at the end of that lane,
                        // and then reroute when react_before_lookahead is called later.
                        return Some(on.length(map));
                    }
                }
                Goal::FollowBusRoute => {
                    return Some(transit_sim.get_dist_to_stop_at(vehicle.id, on.as_lane()));
                }
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
        let choices = map.get_next_turns_and_lanes(last_lane);
        if choices.is_empty() {
            if view.debug {
                debug!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever", view.id, last_lane);
            }
            return Some(Action::VanishAtDeadEnd);
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

    pub fn trace_route(&self, start_dist: Distance, map: &Map, dist_ahead: Distance) -> Trace {
        self.path.trace(map, start_dist, dist_ahead)
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }
}

fn find_parking_spot(
    driving_lane: LaneID,
    dist_along: Distance,
    map: &Map,
    parking_sim: &ParkingSimState,
) -> Option<ParkingSpot> {
    map.get_parent(driving_lane)
        .find_parking_lane(driving_lane)
        .ok()
        .and_then(|l| parking_sim.get_first_free_spot(l, dist_along))
}
