mod driving;
mod slow;
mod walking;

use self::driving::{Outcome, VehiclePathfinder};
use self::walking::SidewalkPathfinder;
use crate::{BusRouteID, BusStopID, LaneID, LaneType, Map, Position, Traversable, TurnID};
use geom::{Distance, PolyLine};
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;

pub type Trace = PolyLine;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PathStep {
    // Original direction
    Lane(LaneID),
    // Sidewalks only!
    ContraflowLane(LaneID),
    Turn(TurnID),
}

impl PathStep {
    pub fn is_contraflow(&self) -> bool {
        match self {
            PathStep::ContraflowLane(_) => true,
            _ => false,
        }
    }

    pub fn as_traversable(&self) -> Traversable {
        match self {
            PathStep::Lane(id) => Traversable::Lane(*id),
            PathStep::ContraflowLane(id) => Traversable::Lane(*id),
            PathStep::Turn(id) => Traversable::Turn(*id),
        }
    }

    pub fn as_lane(&self) -> LaneID {
        self.as_traversable().as_lane()
    }

    pub fn as_turn(&self) -> TurnID {
        self.as_traversable().as_turn()
    }

    // Returns dist_remaining. start is relative to the start of the actual geometry -- so from the
    // lane's real start for ContraflowLane.
    fn slice(
        &self,
        map: &Map,
        start: Distance,
        dist_ahead: Option<Distance>,
    ) -> Option<(PolyLine, Distance)> {
        if let Some(d) = dist_ahead {
            if d < Distance::ZERO {
                panic!("Negative dist_ahead?! {}", d);
            }
            if d == Distance::ZERO {
                return None;
            }
        }

        match self {
            PathStep::Lane(id) => {
                let pts = &map.get_l(*id).lane_center_pts;
                if let Some(d) = dist_ahead {
                    pts.slice(start, start + d)
                } else {
                    pts.slice(start, pts.length())
                }
            }
            PathStep::ContraflowLane(id) => {
                let pts = map.get_l(*id).lane_center_pts.reversed();
                let reversed_start = pts.length() - start;
                if let Some(d) = dist_ahead {
                    pts.slice(reversed_start, reversed_start + d)
                } else {
                    pts.slice(reversed_start, pts.length())
                }
            }
            PathStep::Turn(id) => {
                let pts = &map.get_t(*id).geom;
                if let Some(d) = dist_ahead {
                    pts.slice(start, start + d)
                } else {
                    pts.slice(start, pts.length())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Path {
    steps: VecDeque<PathStep>,
    end_dist: Distance,
}

impl Path {
    // TODO pub for DrawCarInput... bleh.
    pub fn new(map: &Map, steps: Vec<PathStep>, end_dist: Distance) -> Path {
        // Can disable this after trusting it.
        validate(map, &steps);
        Path {
            steps: VecDeque::from(steps),
            end_dist,
        }
    }

    pub fn num_lanes(&self) -> usize {
        let mut count = 0;
        for s in &self.steps {
            match s {
                PathStep::Lane(_) | PathStep::ContraflowLane(_) => count += 1,
                _ => {}
            };
        }
        count
    }

    pub fn is_last_step(&self) -> bool {
        self.steps.len() == 1
    }

    pub fn isnt_last_step(&self) -> bool {
        self.steps.len() > 1
    }

    pub fn shift(&mut self) -> PathStep {
        self.steps.pop_front().unwrap()
    }

    pub fn add(&mut self, step: PathStep) {
        self.steps.push_back(step);
    }

    pub fn current_step(&self) -> PathStep {
        self.steps[0]
    }

    pub fn next_step(&self) -> PathStep {
        self.steps[1]
    }

    pub fn last_step(&self) -> PathStep {
        self.steps[self.steps.len() - 1]
    }

    // dist_ahead might be unlimited.
    pub fn trace(
        &self,
        map: &Map,
        start_dist: Distance,
        dist_ahead: Option<Distance>,
    ) -> Option<Trace> {
        let mut pts_so_far: Option<PolyLine> = None;
        let mut dist_remaining = dist_ahead;

        if self.steps.len() == 1 {
            let dist = if start_dist < self.end_dist {
                self.end_dist - start_dist
            } else {
                start_dist - self.end_dist
            };
            if let Some(d) = dist_remaining {
                if dist < d {
                    dist_remaining = Some(dist);
                }
            } else {
                dist_remaining = Some(dist);
            }
        }

        // Special case the first step.
        if let Some((pts, dist)) = self.steps[0].slice(map, start_dist, dist_remaining) {
            pts_so_far = Some(pts);
            if dist_remaining.is_some() {
                dist_remaining = Some(dist);
            }
        }

        if self.steps.len() == 1 {
            // It's possible there are paths on their last step that're effectively empty, because
            // they're a 0-length turn, or something like a pedestrian crossing a front path and
            // immediately getting on a bike.
            return pts_so_far;
        }

        // Crunch through the intermediate steps, as long as we can.
        for i in 1..self.steps.len() {
            if let Some(d) = dist_remaining {
                if d <= Distance::ZERO {
                    // We know there's at least some geometry if we made it here, so unwrap to verify
                    // that understanding.
                    return Some(pts_so_far.unwrap());
                }
            }
            // If we made it to the last step, maybe use the end_dist.
            if i == self.steps.len() - 1 {
                let end_dist = match self.steps[i] {
                    PathStep::ContraflowLane(l) => {
                        map.get_l(l).lane_center_pts.reversed().length() - self.end_dist
                    }
                    _ => self.end_dist,
                };
                if let Some(d) = dist_remaining {
                    if end_dist < d {
                        dist_remaining = Some(end_dist);
                    }
                } else {
                    dist_remaining = Some(end_dist);
                }
            }

            let start_dist_this_step = match self.steps[i] {
                // TODO Length of a PolyLine can slightly change when points are reversed! That
                // seems bad.
                PathStep::ContraflowLane(l) => map.get_l(l).lane_center_pts.reversed().length(),
                _ => Distance::ZERO,
            };
            if let Some((new_pts, dist)) =
                self.steps[i].slice(map, start_dist_this_step, dist_remaining)
            {
                if pts_so_far.is_some() {
                    let pts = pts_so_far.unwrap().extend(&new_pts);
                    pts_so_far = Some(pts);
                } else {
                    pts_so_far = Some(new_pts);
                }
                if dist_remaining.is_some() {
                    dist_remaining = Some(dist);
                }
            }
        }

        Some(pts_so_far.unwrap())
    }

    pub fn get_steps(&self) -> &VecDeque<PathStep> {
        &self.steps
    }
}

#[derive(Debug, Clone)]
pub struct PathRequest {
    pub start: Position,
    pub end: Position,
    pub can_use_bike_lanes: bool,
    pub can_use_bus_lanes: bool,
}

fn validate(map: &Map, steps: &Vec<PathStep>) {
    if steps.is_empty() {
        panic!("Empty Path");
    }
    for pair in steps.windows(2) {
        let from = match pair[0] {
            PathStep::Lane(id) => map.get_l(id).last_pt(),
            PathStep::ContraflowLane(id) => map.get_l(id).first_pt(),
            PathStep::Turn(id) => map.get_t(id).geom.last_pt(),
        };
        let to = match pair[1] {
            PathStep::Lane(id) => map.get_l(id).first_pt(),
            PathStep::ContraflowLane(id) => map.get_l(id).last_pt(),
            PathStep::Turn(id) => map.get_t(id).geom.first_pt(),
        };
        let len = from.dist_to(to);
        if len > Distance::ZERO {
            println!("All steps in invalid path:");
            for s in steps {
                match s {
                    PathStep::Lane(l) => println!(
                        "  {:?} from {} to {}",
                        s,
                        map.get_l(*l).src_i,
                        map.get_l(*l).dst_i
                    ),
                    PathStep::ContraflowLane(l) => println!(
                        "  {:?} from {} to {}",
                        s,
                        map.get_l(*l).dst_i,
                        map.get_l(*l).src_i
                    ),
                    PathStep::Turn(_) => println!("  {:?}", s),
                }
            }
            panic!(
                "pathfind() returned path that warps {} from {:?} to {:?}",
                len, pair[0], pair[1]
            );
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Pathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    walking_with_transit_graph: SidewalkPathfinder,
}

impl Pathfinder {
    pub fn new(map: &Map) -> Pathfinder {
        Pathfinder {
            car_graph: VehiclePathfinder::new(map, vec![LaneType::Driving]),
            bike_graph: VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Biking]),
            bus_graph: VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Bus]),
            walking_graph: SidewalkPathfinder::new(map, false),
            walking_with_transit_graph: SidewalkPathfinder::new(map, true),
        }
    }

    // TODO tmp
    pub fn shortest_distance(map: &Map, req: PathRequest) -> Option<Path> {
        slow::shortest_distance(map, req)
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        if req.start == req.end {
            panic!("Bad request {:?}", req);
        }

        let outcome = if map.get_l(req.start.lane()).is_sidewalk() {
            match self.walking_graph.pathfind(&req, map) {
                Some(path) => Outcome::Success(path),
                None => Outcome::Failure,
            }
        } else if req.can_use_bus_lanes {
            self.bus_graph.pathfind(&req, map)
        } else if req.can_use_bike_lanes {
            self.bike_graph.pathfind(&req, map)
        } else {
            self.car_graph.pathfind(&req, map)
        };
        match outcome {
            //Outcome::Success(path) => Some(path),
            Outcome::Success(path) => {
                let ok1 = match path.current_step().as_traversable() {
                    Traversable::Lane(l) => l == req.start.lane(),
                    Traversable::Turn(t) => t.src == req.start.lane(),
                };
                let ok2 = match path.last_step().as_traversable() {
                    Traversable::Lane(l) => l == req.end.lane(),
                    Traversable::Turn(t) => t.dst == req.end.lane(),
                };
                if !ok1 || !ok2 {
                    println!("request is {:?}", req);
                    for step in path.get_steps() {
                        println!("- {:?}", step);
                    }
                    panic!(
                        "bad path starting on a {:?}",
                        map.get_l(req.start.lane()).lane_type
                    );
                }

                Some(path)
            }
            Outcome::Failure => None,
            Outcome::RetrySlow => slow::shortest_distance(map, req),
        }
    }

    pub fn should_use_transit(
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        slow::should_use_transit(map, start, end)
    }

    pub fn new_should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        self.walking_with_transit_graph
            .should_use_transit(map, start, end)
    }
}
