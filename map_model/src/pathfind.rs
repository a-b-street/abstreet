use dimensioned::si;
use geom::{Line, PolyLine, Pt2D};
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use {BusStopID, LaneID, LaneType, Map, Position, Traversable, TurnID};

pub type Trace = PolyLine;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PathStep {
    // Original direction
    Lane(LaneID),
    // Sidewalks only!
    ContraflowLane(LaneID),
    Turn(TurnID),
}

// TODO This is like PathStep, but also encodes the possibility of taking a bus.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
enum InternalPathStep {
    Lane(LaneID),
    ContraflowLane(LaneID),
    Turn(TurnID),
    // TODO It'd be great to assign a RouteID in the map layer and not clone a string constantly.
    RideBus(BusStopID, BusStopID, String),
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

    // Returns dist_remaining. start is relative to the start of the actual geometry -- so from the
    // lane's real start for ContraflowLane.
    fn slice(
        &self,
        map: &Map,
        start: si::Meter<f64>,
        dist_ahead: si::Meter<f64>,
    ) -> Option<(PolyLine, si::Meter<f64>)> {
        if dist_ahead < 0.0 * si::M {
            panic!("Negative dist_ahead?! {}", dist_ahead);
        }
        if dist_ahead == 0.0 * si::M {
            return None;
        }

        match self {
            PathStep::Lane(id) => {
                let l = map.get_l(*id);
                // Might have a pedestrian at a front_path lined up with the end of a lane
                if start == l.length() {
                    None
                } else {
                    Some(l.lane_center_pts.slice(start, start + dist_ahead))
                }
            }
            PathStep::ContraflowLane(id) => {
                if start == 0.0 * si::M {
                    None
                } else {
                    let pts = map.get_l(*id).lane_center_pts.reversed();
                    let reversed_start = pts.length() - start;
                    Some(pts.slice(reversed_start, reversed_start + dist_ahead))
                }
            }
            PathStep::Turn(id) => {
                let line = &map.get_t(*id).line;
                if line.length() == 0.0 * si::M {
                    None
                } else {
                    Some(
                        PolyLine::new(vec![line.pt1(), line.pt2()])
                            .slice(start, start + dist_ahead),
                    )
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Path {
    steps: VecDeque<PathStep>,
    end_dist: si::Meter<f64>,
}

impl Path {
    fn new(map: &Map, steps: Vec<PathStep>, end_dist: si::Meter<f64>) -> Path {
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

    pub fn trace(
        &self,
        map: &Map,
        start_dist: si::Meter<f64>,
        dist_ahead: si::Meter<f64>,
    ) -> Option<Trace> {
        let mut pts_so_far: Option<PolyLine> = None;
        let mut dist_remaining = dist_ahead;

        if self.steps.len() == 1 {
            let dist = if start_dist < self.end_dist {
                self.end_dist - start_dist
            } else {
                start_dist - self.end_dist
            };
            if dist < dist_remaining {
                dist_remaining = dist;
            }
        }

        // Special case the first step.
        if let Some((pts, dist)) = self.steps[0].slice(map, start_dist, dist_remaining) {
            pts_so_far = Some(pts);
            dist_remaining = dist;
        }

        if self.steps.len() == 1 {
            // It's possible there are paths on their last step that're effectively empty, because
            // they're a 0-length turn, or something like a pedestrian crossing a front path and
            // immediately getting on a bike.
            return pts_so_far;
        }

        // Crunch through the intermediate steps, as long as we can.
        for i in 1..self.steps.len() {
            if dist_remaining <= 0.0 * si::M {
                // We know there's at least some geometry if we made it here, so unwrap to verify
                // that understanding.
                return Some(pts_so_far.unwrap());
            }
            // If we made it to the last step, maybe use the end_dist.
            if i == self.steps.len() - 1 && self.end_dist < dist_remaining {
                dist_remaining = self.end_dist;
            }

            let start_dist_this_step = match self.steps[i] {
                // TODO Length of a PolyLine can slightly change when points are reversed! That
                // seems bad.
                PathStep::ContraflowLane(l) => map.get_l(l).lane_center_pts.reversed().length(),
                _ => 0.0 * si::M,
            };
            if let Some((new_pts, dist)) =
                self.steps[i].slice(map, start_dist_this_step, dist_remaining)
            {
                if let Some(ref mut pts) = pts_so_far {
                    pts.extend(new_pts);
                } else {
                    pts_so_far = Some(new_pts);
                }
                dist_remaining = dist;
            }
        }

        return Some(pts_so_far.unwrap());
    }

    pub fn get_steps(&self) -> &VecDeque<PathStep> {
        &self.steps
    }
}

#[derive(Clone)]
pub struct PathRequest {
    pub start: Position,
    pub end: Position,
    pub can_use_bike_lanes: bool,
    pub can_use_bus_lanes: bool,
}

pub struct Pathfinder {
    goal_pt: Pt2D,
    can_use_bike_lanes: bool,
    can_use_bus_lanes: bool,
    can_use_transit: bool,
}

impl Pathfinder {
    // Returns an inclusive path, aka, [start, ..., end]
    pub fn shortest_distance(map: &Map, req: PathRequest) -> Option<Path> {
        // TODO using first_pt here and in heuristic_dist is particularly bad for walking
        // directions
        let goal_pt = req.end.pt(map);
        let internal_steps = Pathfinder {
            goal_pt,
            can_use_bike_lanes: req.can_use_bike_lanes,
            can_use_bus_lanes: req.can_use_bus_lanes,
            can_use_transit: false,
        }.pathfind(map, req.start, req.end)?;
        let steps: Vec<PathStep> = internal_steps
            .into_iter()
            .map(|s| match s {
                InternalPathStep::Lane(l) => PathStep::Lane(l),
                InternalPathStep::ContraflowLane(l) => PathStep::ContraflowLane(l),
                InternalPathStep::Turn(t) => PathStep::Turn(t),
                InternalPathStep::RideBus(_, _, _) => {
                    panic!("shortest_distance pathfind had {:?} as a step", s)
                }
            }).collect();
        assert_eq!(
            steps[0].as_traversable(),
            Traversable::Lane(req.start.lane())
        );
        assert_eq!(
            steps.last().unwrap().as_traversable(),
            Traversable::Lane(req.end.lane())
        );
        return Some(Path::new(map, steps, req.end.dist_along()));
    }

    // Attempt the pathfinding and see if riding a bus is a step.
    pub fn should_use_transit(
        map: &Map,
        req: PathRequest,
    ) -> Option<(BusStopID, BusStopID, String)> {
        // TODO using first_pt here and in heuristic_dist is particularly bad for walking
        // directions
        let goal_pt = req.end.pt(map);
        let internal_steps = Pathfinder {
            goal_pt,
            can_use_bike_lanes: false,
            can_use_bus_lanes: false,
            can_use_transit: true,
        }.pathfind(map, req.start, req.end)?;
        for s in internal_steps.into_iter() {
            if let InternalPathStep::RideBus(stop1, stop2, route) = s {
                return Some((stop1, stop2, route));
            }
        }
        None
    }

    // Returns the cost of the potential next step, plus an optional heuristic to the goal
    // TODO Do this cost/heuristic thing somewhere else.
    fn expand(&self, map: &Map, current: InternalPathStep) -> Vec<(InternalPathStep, f64)> {
        let mut results: Vec<(InternalPathStep, f64)> = Vec::new();
        match current {
            InternalPathStep::Lane(l) | InternalPathStep::ContraflowLane(l) => {
                let endpoint = if current == InternalPathStep::Lane(l) {
                    map.get_l(l).dst_i
                } else {
                    map.get_l(l).src_i
                };
                for (turn, next) in map.get_next_turns_and_lanes(l, endpoint).into_iter() {
                    if !self.can_use_bike_lanes && next.lane_type == LaneType::Biking {
                        // Skip
                    } else if !self.can_use_bus_lanes && next.lane_type == LaneType::Bus {
                        // Skip
                    } else {
                        let cost = turn.length();
                        let heuristic = Line::new(turn.last_pt(), self.goal_pt).length();
                        results.push((
                            InternalPathStep::Turn(turn.id),
                            (cost + heuristic).value_unsafe,
                        ));
                    }
                }

                if self.can_use_transit {
                    for stop1 in &map.get_l(l).bus_stops {
                        for (stop2, route) in map.get_connected_bus_stops(*stop1).into_iter() {
                            // No cost for riding the bus, for now.
                            let heuristic =
                                Line::new(map.get_bs(stop2).sidewalk_pos.pt(map), self.goal_pt)
                                    .length();
                            results.push((
                                InternalPathStep::RideBus(*stop1, stop2, route),
                                heuristic.value_unsafe,
                            ));
                        }
                    }
                }
            }
            InternalPathStep::Turn(t) => {
                let dst = map.get_l(t.dst);
                let cost = dst.length();
                if t.parent == dst.src_i {
                    let heuristic = Line::new(dst.last_pt(), self.goal_pt).length();
                    results.push((
                        InternalPathStep::Lane(dst.id),
                        (cost + heuristic).value_unsafe,
                    ));
                } else {
                    let heuristic = Line::new(dst.first_pt(), self.goal_pt).length();
                    results.push((
                        InternalPathStep::ContraflowLane(dst.id),
                        (cost + heuristic).value_unsafe,
                    ));
                }
            }
            InternalPathStep::RideBus(_, stop2, _) => {
                let pos = map.get_bs(stop2).sidewalk_pos;
                let sidewalk = map.get_l(pos.lane());
                if pos.dist_along() != sidewalk.length() {
                    let cost = sidewalk.length() - pos.dist_along();
                    let heuristic = Line::new(sidewalk.last_pt(), self.goal_pt).length();
                    results.push((
                        InternalPathStep::Lane(sidewalk.id),
                        (cost + heuristic).value_unsafe,
                    ));
                }
                if pos.dist_along() != 0.0 * si::M {
                    let cost = pos.dist_along();
                    let heuristic = Line::new(sidewalk.first_pt(), self.goal_pt).length();
                    results.push((
                        InternalPathStep::ContraflowLane(sidewalk.id),
                        (cost + heuristic).value_unsafe,
                    ));
                }
            }
        };
        results
    }

    fn pathfind(&self, map: &Map, start: Position, end: Position) -> Option<Vec<InternalPathStep>> {
        if start.lane() == end.lane() {
            if start.dist_along() > end.dist_along() {
                assert_eq!(map.get_l(start.lane()).lane_type, LaneType::Sidewalk);
                return Some(vec![InternalPathStep::ContraflowLane(start.lane())]);
            }
            return Some(vec![InternalPathStep::Lane(start.lane())]);
        }

        // This should be deterministic, since cost ties would be broken by PathStep.
        let mut queue: BinaryHeap<(NotNaN<f64>, InternalPathStep)> = BinaryHeap::new();
        if map.get_l(start.lane()).is_sidewalk() {
            if start.dist_along() != map.get_l(start.lane()).length() {
                queue.push((
                    NotNaN::new(-0.0).unwrap(),
                    InternalPathStep::Lane(start.lane()),
                ));
            }
            if start.dist_along() != 0.0 * si::M {
                queue.push((
                    NotNaN::new(-0.0).unwrap(),
                    InternalPathStep::ContraflowLane(start.lane()),
                ));
            }
        } else {
            queue.push((
                NotNaN::new(-0.0).unwrap(),
                InternalPathStep::Lane(start.lane()),
            ));
        }

        let mut backrefs: HashMap<InternalPathStep, InternalPathStep> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == InternalPathStep::Lane(end.lane())
                || current == InternalPathStep::ContraflowLane(end.lane())
            {
                let mut reversed_steps: Vec<InternalPathStep> = Vec::new();
                let mut lookup = current;
                loop {
                    reversed_steps.push(lookup.clone());
                    if lookup == InternalPathStep::Lane(start.lane())
                        || lookup == InternalPathStep::ContraflowLane(start.lane())
                    {
                        reversed_steps.reverse();
                        return Some(reversed_steps);
                    }
                    lookup = backrefs[&lookup].clone();
                }
            }

            // Expand
            for (next, cost) in self.expand(map, current.clone()).into_iter() {
                if !backrefs.contains_key(&next) {
                    backrefs.insert(next.clone(), current.clone());
                    // Negate since BinaryHeap is a max-heap.
                    queue.push((
                        NotNaN::new(-1.0).unwrap() * (NotNaN::new(cost).unwrap() + cost_sofar),
                        next,
                    ));
                }
            }
        }

        // No path
        None
    }
}

fn validate(map: &Map, steps: &Vec<PathStep>) {
    for pair in steps.windows(2) {
        let from = match pair[0] {
            PathStep::Lane(id) => map.get_l(id).last_pt(),
            PathStep::ContraflowLane(id) => map.get_l(id).first_pt(),
            PathStep::Turn(id) => map.get_t(id).last_pt(),
        };
        let to = match pair[1] {
            PathStep::Lane(id) => map.get_l(id).first_pt(),
            PathStep::ContraflowLane(id) => map.get_l(id).last_pt(),
            PathStep::Turn(id) => map.get_t(id).first_pt(),
        };
        let len = Line::new(from, to).length();
        if len > 0.0 * si::M {
            error!("All steps in invalid path:");
            for s in steps {
                match s {
                    PathStep::Lane(l) => error!(
                        "  {:?} from {} to {}",
                        s,
                        map.get_l(*l).src_i,
                        map.get_l(*l).dst_i
                    ),
                    PathStep::ContraflowLane(l) => error!(
                        "  {:?} from {} to {}",
                        s,
                        map.get_l(*l).dst_i,
                        map.get_l(*l).src_i
                    ),
                    PathStep::Turn(_) => error!("  {:?}", s),
                }
            }
            panic!(
                "pathfind() returned path that warps {} from {:?} to {:?}",
                len, pair[0], pair[1]
            );
        }
    }
}
