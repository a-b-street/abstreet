use dimensioned::si;
use geom::{Line, PolyLine, Pt2D};
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use {LaneID, LaneType, Map, Traversable, TurnID};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
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

    // Returns dist_remaining
    fn slice(
        &self,
        map: &Map,
        start: si::Meter<f64>,
        dist_ahead: si::Meter<f64>,
    ) -> Option<(PolyLine, si::Meter<f64>)> {
        if dist_ahead < 0.0 * si::M {
            panic!("Negative dist_ahead?! {}", dist_ahead);
        }

        match self {
            PathStep::Lane(id) => Some(
                map.get_l(*id)
                    .lane_center_pts
                    .slice(start, start + dist_ahead),
            ),
            PathStep::ContraflowLane(id) => {
                let pts = map.get_l(*id).lane_center_pts.reversed();
                let reversed_start = pts.length() - start;
                Some(pts.slice(reversed_start, reversed_start + dist_ahead))
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

#[derive(Debug, Clone, Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Path {
    steps: VecDeque<PathStep>,
    // TODO :(
    #[derivative(PartialEq = "ignore")]
    end_dist: si::Meter<f64>,
}

// TODO can have a method to verify the path is valid
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
    ) -> Trace {
        let mut pts_so_far: Option<PolyLine> = None;
        let mut dist_remaining = dist_ahead;

        fn extend(a: &mut Option<PolyLine>, b: PolyLine) {
            if let Some(ref mut pts) = a {
                pts.extend(b);
            } else {
                *a = Some(b);
            }
        }

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
            // TODO uh, there's one case where this won't work
            return pts_so_far.unwrap();
        }

        // Crunch through the intermediate steps, as long as we can.
        for i in 1..self.steps.len() {
            if dist_remaining <= 0.0 * si::M {
                return pts_so_far.unwrap();
            }
            // If we made it to the last step, maybe use the end_dist.
            if i == self.steps.len() - 1 && self.end_dist < dist_remaining {
                dist_remaining = self.end_dist;
            }

            if let Some((pts, dist)) = self.steps[i].slice(map, 0.0 * si::M, dist_remaining) {
                extend(&mut pts_so_far, pts);
                dist_remaining = dist;
            }
        }

        return pts_so_far.unwrap();
    }

    pub fn get_steps(&self) -> &VecDeque<PathStep> {
        &self.steps
    }
}

pub enum Pathfinder {
    ShortestDistance {
        goal_pt: Pt2D,
        can_use_bike_lanes: bool,
        can_use_bus_lanes: bool,
    },
    UsingTransit,
}

impl Pathfinder {
    // Returns an inclusive path, aka, [start, ..., end]
    pub fn shortest_distance(
        map: &Map,
        start: LaneID,
        start_dist: si::Meter<f64>,
        end: LaneID,
        end_dist: si::Meter<f64>,
        // TODO ew, bools.
        can_use_bike_lanes: bool,
        can_use_bus_lanes: bool,
    ) -> Option<Path> {
        // TODO using first_pt here and in heuristic_dist is particularly bad for walking
        // directions
        let goal_pt = map.get_l(end).first_pt();
        Pathfinder::ShortestDistance {
            goal_pt,
            can_use_bike_lanes,
            can_use_bus_lanes,
        }.pathfind(map, start, start_dist, end, end_dist)
    }

    fn expand(&self, map: &Map, current: LaneID) -> Vec<(LaneID, NotNaN<f64>)> {
        match self {
            Pathfinder::ShortestDistance {
                goal_pt,
                can_use_bike_lanes,
                can_use_bus_lanes,
            } => {
                let current_length = NotNaN::new(map.get_l(current).length().value_unsafe).unwrap();
                map.get_next_turns_and_lanes(current)
                    .into_iter()
                    .filter_map(|(_, next)| {
                        if !can_use_bike_lanes && next.lane_type == LaneType::Biking {
                            None
                        } else if !can_use_bus_lanes && next.lane_type == LaneType::Bus {
                            None
                        } else {
                            // TODO cost and heuristic are wrong. need to reason about PathSteps,
                            // not LaneIDs, I think. :\
                            let heuristic_dist = NotNaN::new(
                                Line::new(next.first_pt(), *goal_pt).length().value_unsafe,
                            ).unwrap();
                            Some((next.id, current_length + heuristic_dist))
                        }
                    }).collect()
            }
            Pathfinder::UsingTransit => {
                // No heuristic, because it's hard to make admissible.
                // Cost is distance spent walking, so any jumps made using a bus are FREE. This is
                // unrealistic, but a good way to start exercising peds using transit.
                let current_lane = map.get_l(current);
                let current_length = NotNaN::new(current_lane.length().value_unsafe).unwrap();
                let mut results: Vec<(LaneID, NotNaN<f64>)> = Vec::new();
                for (_, next) in &map.get_next_turns_and_lanes(current) {
                    results.push((next.id, current_length));
                }
                // TODO Need to add a PathStep for riding a bus between two stops.
                /*
                for stop1 in &current_lane.bus_stops {
                    for stop2 in &map.get_connected_bus_stops(*stop1) {
                        results.push((stop2.sidewalk, current_length));
                    }
                }
                */
                results
            }
        }
    }

    fn pathfind(
        &self,
        map: &Map,
        start: LaneID,
        start_dist: si::Meter<f64>,
        end: LaneID,
        end_dist: si::Meter<f64>,
    ) -> Option<Path> {
        assert_eq!(map.get_l(start).lane_type, map.get_l(end).lane_type);
        if start == end {
            if start_dist > end_dist {
                assert_eq!(map.get_l(start).lane_type, LaneType::Sidewalk);
                return Some(Path::new(
                    map,
                    vec![PathStep::ContraflowLane(start)],
                    end_dist,
                ));
            }
            return Some(Path::new(map, vec![PathStep::Lane(start)], end_dist));
        }

        // This should be deterministic, since cost ties would be broken by LaneID.
        let mut queue: BinaryHeap<(NotNaN<f64>, LaneID)> = BinaryHeap::new();
        queue.push((NotNaN::new(-0.0).unwrap(), start));

        let mut backrefs: HashMap<LaneID, LaneID> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == end {
                let mut reversed_lanes: Vec<LaneID> = Vec::new();
                let mut lookup = current;
                loop {
                    reversed_lanes.push(lookup);
                    if lookup == start {
                        reversed_lanes.reverse();
                        assert_eq!(reversed_lanes[0], start);
                        assert_eq!(*reversed_lanes.last().unwrap(), end);
                        return Some(lanes_to_path(map, VecDeque::from(reversed_lanes), end_dist));
                    }
                    lookup = backrefs[&lookup];
                }
            }

            // Expand
            for (next, cost) in self.expand(map, current).into_iter() {
                if !backrefs.contains_key(&next) {
                    backrefs.insert(next, current);
                    // Negate since BinaryHeap is a max-heap.
                    queue.push((NotNaN::new(-1.0).unwrap() * (cost + cost_sofar), next));
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
            panic!(
                "pathfind() returned path that warps {} from {:?} to {:?}",
                len, pair[0], pair[1]
            );
        }
    }
}

// TODO Tmp hack. Need to rewrite the A* implementation to natively understand PathSteps.
fn lanes_to_path(map: &Map, mut lanes: VecDeque<LaneID>, end_dist: si::Meter<f64>) -> Path {
    assert!(lanes.len() > 1);
    let mut steps: Vec<PathStep> = Vec::new();

    if is_contraflow(map, lanes[0], lanes[1]) {
        steps.push(PathStep::ContraflowLane(lanes[0]));
    } else {
        steps.push(PathStep::Lane(lanes[0]));
    }
    let mut current_turn = pick_turn(lanes[0], lanes[1], map);
    steps.push(PathStep::Turn(current_turn));

    lanes.pop_front();
    lanes.pop_front();

    loop {
        if lanes.is_empty() {
            break;
        }

        assert!(lanes[0] != current_turn.dst);

        let next_turn = pick_turn(current_turn.dst, lanes[0], map);
        if current_turn.parent == next_turn.parent {
            // Don't even cross the current lane!
        } else if leads_to_end_of_lane(current_turn, map) {
            steps.push(PathStep::ContraflowLane(current_turn.dst));
        } else {
            steps.push(PathStep::Lane(current_turn.dst));
        }
        steps.push(PathStep::Turn(next_turn));

        lanes.pop_front();
        current_turn = next_turn;
    }

    if leads_to_end_of_lane(current_turn, map) {
        steps.push(PathStep::ContraflowLane(current_turn.dst));
    } else {
        steps.push(PathStep::Lane(current_turn.dst));
    }
    Path::new(map, steps, end_dist)
}

fn pick_turn(from: LaneID, to: LaneID, map: &Map) -> TurnID {
    let l = map.get_l(from);
    let endpoint = if is_contraflow(map, from, to) {
        l.src_i
    } else {
        l.dst_i
    };

    for t in map.get_turns_from_lane(from) {
        if t.id.parent == endpoint && t.id.dst == to {
            return t.id;
        }
    }
    panic!("No turn from {} ({} end) to {}", from, endpoint, to);
}

fn is_contraflow(map: &Map, from: LaneID, to: LaneID) -> bool {
    map.get_l(from).dst_i != map.get_l(to).src_i
}

fn leads_to_end_of_lane(turn: TurnID, map: &Map) -> bool {
    is_contraflow(map, turn.src, turn.dst)
}

pub type Trace = PolyLine;
