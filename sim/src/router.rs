use dimensioned::si;
use driving::Action;
use kinematics;
use kinematics::Vehicle;
use map_model::{BuildingID, LaneID, Map, Trace, Traversable, TurnID};
use parking::ParkingSimState;
use rand::{Rng, XorShiftRng};
use std::collections::VecDeque;
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
    // Head is the next lane, so when the car finishes a turn and enters the next lane, this
    // shifts.
    path: VecDeque<LaneID>,
    goal: Goal,
}

impl Router {
    pub fn make_router_to_park(path: VecDeque<LaneID>, goal: BuildingID) -> Router {
        Router {
            path,
            goal: Goal::ParkNearBuilding(goal),
        }
    }

    pub fn make_router_for_bus(first_path: VecDeque<LaneID>) -> Router {
        Router {
            path: first_path,
            goal: Goal::FollowBusRoute,
        }
    }

    pub fn tooltip_line(&self) -> String {
        format!("{} lanes left in path", self.path.len())
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
        if !self.path.is_empty() || view.speed > kinematics::EPSILON_SPEED {
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
        if self.path.is_empty() {
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

    pub fn choose_turn(&self, from: LaneID, map: &Map) -> TurnID {
        pick_turn(from, self.path[0], map)
    }

    pub fn advance_to(&mut self, next_lane: LaneID) {
        assert_eq!(next_lane, self.path[0]);
        self.path.pop_front();
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
        let choices = map.get_next_lanes(last_lane);
        if choices.is_empty() {
            if view.debug {
                debug!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever", view.id, last_lane);
            }
            return Some(Action::VanishAtDeadEnd);
        }
        let choice = rng.choose(&choices).unwrap().id;
        if view.debug {
            debug!(
                "{} can't find parking on {}, so wandering over to {}",
                view.id, last_lane, choice
            );
        }
        self.path.push_back(choice);
        None
    }

    pub fn trace_route(
        &self,
        start: Traversable,
        start_dist: Distance,
        map: &Map,
        dist_along: Distance,
    ) -> Trace {
        let (mut result, mut dist_left) =
            start.slice(false, map, start_dist, start_dist + dist_along);

        let mut last_lane = start.maybe_lane();
        let mut idx = 0;
        while dist_left > 0.0 * si::M && idx < self.path.len() {
            let next_lane = self.path[idx];
            if let Some(prev) = last_lane {
                let (piece, new_dist_left) = Traversable::Turn(pick_turn(prev, next_lane, map))
                    .slice(false, map, 0.0 * si::M, dist_left);
                result = result.extend(piece);
                dist_left = new_dist_left;
                if dist_left <= 0.0 * si::M {
                    break;
                }
            }

            let (piece, new_dist_left) =
                Traversable::Lane(next_lane).slice(false, map, 0.0 * si::M, dist_left);
            if piece.endpoints().0 != result.endpoints().1 {
                println!("so far");
                result.debug();
                println!("new piece");
                piece.debug();
            }
            result = result.extend(piece);
            dist_left = new_dist_left;
            last_lane = Some(next_lane);

            idx += 1;
        }

        // Excess dist_left is just ignored
        result
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

fn pick_turn(from: LaneID, to: LaneID, map: &Map) -> TurnID {
    for t in map.get_turns_from_lane(from) {
        if t.dst == to {
            return t.id;
        }
    }
    panic!("No turn from {} to {}", from, to);
}
