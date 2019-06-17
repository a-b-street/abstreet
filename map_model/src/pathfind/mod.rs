mod driving;
mod node_map;
mod slow;
mod walking;

use self::driving::{Outcome, VehiclePathfinder};
use self::walking::SidewalkPathfinder;
use crate::{BusRouteID, BusStopID, LaneID, LaneType, Map, Position, Traversable, TurnID};
use abstutil::Timer;
use geom::{Distance, PolyLine};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use std::fmt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathStep {
    // Original direction
    Lane(LaneID),
    // Sidewalks only!
    ContraflowLane(LaneID),
    Turn(TurnID),
}

impl PathStep {
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

    // dist_ahead is unlimited when None.
    pub fn trace(
        &self,
        map: &Map,
        start_dist: Distance,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
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
                    pts_so_far = Some(pts_so_far.unwrap().extend(new_pts));
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

    pub fn total_dist(&self, map: &Map) -> Distance {
        let mut dist = Distance::ZERO;
        for s in &self.steps {
            dist += s.as_traversable().length(map);
        }
        dist
    }
}

#[derive(Clone)]
pub struct PathRequest {
    pub start: Position,
    pub end: Position,
    pub can_use_bike_lanes: bool,
    pub can_use_bus_lanes: bool,
}

impl fmt::Display for PathRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PathRequest({} along {}... to {} along {}",
            self.start.dist_along(),
            self.start.lane(),
            self.end.dist_along(),
            self.end.lane()
        )?;
        // TODO can_use_bike_lanes and can_use_bus_lanes are mutex, encode that directly.
        if self.can_use_bike_lanes {
            write!(f, ", bike lanes)")
        } else if self.can_use_bus_lanes {
            write!(f, ", bus lanes)")
        } else {
            write!(f, ")")
        }
    }
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

#[derive(Serialize, Deserialize)]
pub struct Pathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    // TODO Option just during initialization! Ewww.
    walking_with_transit_graph: Option<SidewalkPathfinder>,
}

impl Pathfinder {
    pub fn new_without_transit(map: &Map, timer: &mut Timer) -> Pathfinder {
        timer.start("prepare pathfinding for cars");
        let car_graph = VehiclePathfinder::new(map, vec![LaneType::Driving]);
        timer.stop("prepare pathfinding for cars");

        timer.start("prepare pathfinding for bikes");
        let bike_graph = VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Biking]);
        timer.stop("prepare pathfinding for bikes");

        timer.start("prepare pathfinding for buses");
        let bus_graph = VehiclePathfinder::new(map, vec![LaneType::Driving, LaneType::Bus]);
        timer.stop("prepare pathfinding for buses");

        timer.start("prepare pathfinding for pedestrians");
        let walking_graph = SidewalkPathfinder::new(map, false);
        timer.stop("prepare pathfinding for pedestrians");

        Pathfinder {
            car_graph,
            bike_graph,
            bus_graph,
            walking_graph,
            walking_with_transit_graph: None,
        }
    }

    pub fn setup_walking_with_transit(&mut self, map: &Map) {
        self.walking_with_transit_graph = Some(SidewalkPathfinder::new(map, true));
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        // Weird case, but it can happen for walking from a building path to a bus stop that're
        // actually at the same spot.
        if req.start == req.end {
            return Some(Path::new(
                map,
                vec![PathStep::Lane(req.start.lane())],
                req.start.dist_along(),
            ));
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
            Outcome::Success(path) => Some(path),
            Outcome::Failure => None,
            Outcome::RetrySlow => self::slow::shortest_distance(map, req),
        }
    }

    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        self.walking_with_transit_graph
            .as_ref()
            .unwrap()
            .should_use_transit(map, start, end)
    }

    pub fn apply_edits(
        &mut self,
        delete_turns: &BTreeSet<TurnID>,
        add_turns: &BTreeSet<TurnID>,
        map: &Map,
        timer: &mut Timer,
    ) {
        self.car_graph
            .apply_edits(delete_turns, add_turns, map, timer);
        self.bike_graph
            .apply_edits(delete_turns, add_turns, map, timer);
        self.bus_graph
            .apply_edits(delete_turns, add_turns, map, timer);
        // TODO Can edits ever affect walking or walking+transit? If a crosswalk is entirely
        // banned, then yes... but actually that sounds like a bad edit to allow.
    }
}
