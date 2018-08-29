use dimensioned::si;
use driving::{Action, CarView};
use kinematics;
use kinematics::Vehicle;
use map_model;
use map_model::{BuildingID, BusStop, LaneID, Map, TurnID};
use parking::ParkingSimState;
use rand::Rng;
use std::collections::VecDeque;
use {Distance, On, ParkingSpot, Tick};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Goal {
    ParkNearBuilding(BuildingID),
    // This is stateful -- first stop is the one we're aiming for now, and we might also be waiting
    // at that stop for a little while.
    // Buses really have a few states... GotoNextStop(list of stops), WaitAtStop(list of stops,
    // tick)
    CycleThroughStops(Vec<BusStop>, Option<Tick>),
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

    pub fn make_router_for_bus(first_path: VecDeque<LaneID>, stops: Vec<BusStop>) -> Router {
        assert_eq!(*first_path.back().unwrap(), stops[1].driving_lane);
        Router {
            path: first_path,
            goal: Goal::CycleThroughStops(rotate_stops(&stops), None),
        }
    }

    pub fn tooltip_line(&self) -> String {
        format!("{} lanes left in path", self.path.len())
    }

    // Mutable so we can roam around and try another road to park if the last one is unavailable.
    // It's safe to mutate in a react() phase, because we're observing a fixed state of the world
    // and augmenting the plan, but not the car's actual state.
    pub fn react_before_lookahead<R: Rng + ?Sized>(
        &mut self,
        view: &CarView,
        vehicle: &Vehicle,
        time: Tick,
        map: &Map,
        parking_sim: &ParkingSimState,
        rng: &mut R,
    ) -> Option<Action> {
        if self.path.is_empty() && view.speed <= kinematics::EPSILON_SPEED {
            match self.goal {
                Goal::ParkNearBuilding(_) => {
                    let last_lane = view.on.as_lane();
                    if let Some(spot) =
                        find_parking_spot(last_lane, view.dist_along, map, parking_sim)
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
                Goal::CycleThroughStops(ref mut stops, ref mut wait_until) => {
                    if view.dist_along == stops[0].dist_along {
                        if let Some(wait) = wait_until.clone() {
                            if time == wait {
                                if view.debug || true {
                                    println!(
                                        "{} finished waiting at bus stop, going to next stop {:?}",
                                        view.id, stops[1]
                                    );
                                }

                                let mut new_path = VecDeque::from(
                                    map_model::pathfind(
                                        map,
                                        stops[0].driving_lane,
                                        stops[1].driving_lane,
                                    ).expect(&format!(
                                        "No route between bus stops {:?} and {:?}",
                                        stops[0], stops[1]
                                    )),
                                );
                                new_path.pop_front();
                                self.path = new_path;

                                let new_stops = rotate_stops(stops);
                                stops.clear();
                                stops.extend(new_stops);
                                *wait_until = None;
                            }
                        } else {
                            assert_eq!(view.on.as_lane(), stops[0].driving_lane);
                            // TODO const
                            *wait_until = Some(time + 10.0 * si::S);
                            if view.debug || true {
                                println!(
                                    "{} reached {:?}, now waiting until {:?}",
                                    view.id, stops[0], wait_until
                                );
                            }
                            return Some(Action::Continue(0.0 * si::MPS2, Vec::new()));
                        }
                    }
                }
            }
        }
        None
    }

    // If we return None, then the caller will immediately ask what turn to do.
    pub(crate) fn stop_early_at_dist(
        &self,
        // TODO urgh, we cant reuse CarView here, because lookahead doesn't advance the view :(
        on: On,
        dist_along: Distance,
        vehicle: &Vehicle,
        map: &Map,
        parking_sim: &ParkingSimState,
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
                Goal::CycleThroughStops(ref stops, _) => {
                    return Some(stops[0].dist_along);
                }
            }
        }
        None
    }

    pub fn choose_turn(&self, from: LaneID, map: &Map) -> TurnID {
        for t in map.get_turns_from_lane(from) {
            if t.dst == self.path[0] {
                return t.id;
            }
        }
        panic!("No turn from {} to {}", from, self.path[0]);
    }

    pub fn advance_to(&mut self, next_lane: LaneID) {
        assert_eq!(next_lane, self.path[0]);
        self.path.pop_front();
    }

    fn look_for_parking<R: Rng + ?Sized>(
        &mut self,
        last_lane: LaneID,
        view: &CarView,
        map: &Map,
        rng: &mut R,
    ) -> Option<Action> {
        // TODO Better strategies than random: look for lanes with free spots (if it'd be feasible
        // to physically see the spots), stay close to the original goal building, avoid lanes
        // we've visited, prefer easier turns...
        let choices = map.get_next_lanes(last_lane);
        if choices.is_empty() {
            if view.debug {
                println!("{} can't find parking on {}, and also it's a dead-end, so they'll be stuck there forever", view.id, last_lane);
            }
            return Some(Action::VanishAtDeadEnd);
        }
        let choice = rng.choose(&choices).unwrap().id;
        if view.debug {
            println!(
                "{} can't find parking on {}, so wandering over to {}",
                view.id, last_lane, choice
            );
        }
        self.path.push_back(choice);
        None
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
        .and_then(|l| parking_sim.get_first_free_spot(l, dist_along))
}

// TODO double-ended list can do this natively
fn rotate_stops(stops: &Vec<BusStop>) -> Vec<BusStop> {
    let mut cycled_stops: Vec<BusStop> = stops[1..].to_vec();
    cycled_stops.push(stops[0].clone());
    cycled_stops
}
