//! Everything related to pathfinding through a map for different types of agents.

use std::collections::{BTreeSet, VecDeque};
use std::fmt;

use enumset::EnumSetType;
use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::{Distance, PolyLine, EPSILON_DIST};

pub use self::ch::ContractionHierarchyPathfinder;
pub use self::driving::driving_cost;
pub use self::walking::{walking_cost, WalkingNode};
use crate::{
    osm, BusRouteID, BusStopID, Lane, LaneID, LaneType, Map, Position, Traversable, TurnID,
    UberTurn,
};

mod ch;
mod dijkstra;
mod driving;
mod node_map;
// TODO tmp
pub mod uber_turns;
mod walking;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathStep {
    /// Original direction
    Lane(LaneID),
    /// Sidewalks only!
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
    ) -> Result<(PolyLine, Distance), String> {
        if let Some(d) = dist_ahead {
            if d < Distance::ZERO {
                panic!("Negative dist_ahead?! {}", d);
            }
            if d == Distance::ZERO {
                return Err(format!("0 dist ahead for slice"));
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

    // Also track progress along the original path.
    total_length: Distance,
    crossed_so_far: Distance,

    total_lanes: usize,

    // A list of uber-turns encountered by this path, in order. The steps are flattened into the
    // sequence of turn->lane->...->turn.
    uber_turns: VecDeque<UberTurn>,
    // Is the current_step in the middle of an UberTurn?
    currently_inside_ut: Option<UberTurn>,
}

impl Path {
    pub(crate) fn new(
        map: &Map,
        steps: Vec<PathStep>,
        end_dist: Distance,
        uber_turns: Vec<UberTurn>,
    ) -> Path {
        // Haven't seen problems here in a very long time. Noticeably saves some time to skip.
        if false {
            validate_continuity(map, &steps);
        }
        if false {
            validate_restrictions(map, &steps);
        }
        // Slightly expensive, but the contraction hierarchy weights aren't distances.
        let mut total_length = Distance::ZERO;
        let mut total_lanes = 0;
        for s in &steps {
            total_length += s.as_traversable().length(map);
            match s {
                PathStep::Lane(_) | PathStep::ContraflowLane(_) => total_lanes += 1,
                _ => {}
            }
        }
        Path {
            steps: VecDeque::from(steps),
            end_dist,
            total_length,
            crossed_so_far: Distance::ZERO,
            total_lanes,
            uber_turns: uber_turns.into_iter().collect(),
            currently_inside_ut: None,
        }
    }

    pub fn one_step(l: LaneID, map: &Map) -> Path {
        Path::new(
            map,
            vec![PathStep::Lane(l)],
            map.get_l(l).length(),
            Vec::new(),
        )
    }

    /// Only used for weird serialization magic.
    pub fn dummy() -> Path {
        Path {
            steps: VecDeque::new(),
            end_dist: Distance::ZERO,
            total_length: Distance::ZERO,
            crossed_so_far: Distance::ZERO,
            total_lanes: 0,
            uber_turns: VecDeque::new(),
            currently_inside_ut: None,
        }
    }

    pub fn total_lanes(&self) -> usize {
        self.total_lanes
    }

    pub fn crossed_so_far(&self) -> Distance {
        self.crossed_so_far
    }

    pub fn total_length(&self) -> Distance {
        self.total_length
    }

    pub fn percent_dist_crossed(&self) -> f64 {
        // Sometimes this happens
        if self.total_length == Distance::ZERO {
            return 1.0;
        }
        self.crossed_so_far / self.total_length
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn is_last_step(&self) -> bool {
        self.steps.len() == 1
    }

    pub fn isnt_last_step(&self) -> bool {
        self.steps.len() > 1
    }

    pub fn currently_inside_ut(&self) -> &Option<UberTurn> {
        &self.currently_inside_ut
    }
    pub fn about_to_start_ut(&self) -> Option<&UberTurn> {
        if self.steps.len() < 2 || self.uber_turns.is_empty() {
            return None;
        }
        if let PathStep::Turn(t) = self.steps[1] {
            if self.uber_turns[0].path[0] == t {
                return Some(&self.uber_turns[0]);
            }
        }
        None
    }

    pub fn shift(&mut self, map: &Map) -> PathStep {
        let step = self.steps.pop_front().unwrap();
        self.crossed_so_far += step.as_traversable().length(map);

        if let Some(ref ut) = self.currently_inside_ut {
            if step == PathStep::Turn(*ut.path.last().unwrap()) {
                self.currently_inside_ut = None;
            }
        } else if !self.steps.is_empty() && !self.uber_turns.is_empty() {
            if self.steps[0] == PathStep::Turn(self.uber_turns[0].path[0]) {
                self.currently_inside_ut = Some(self.uber_turns.pop_front().unwrap());
            }
        }

        if self.steps.len() == 1 {
            // TODO When handle_uber_turns experiment is turned off, this will crash
            assert!(self.uber_turns.is_empty());
            assert!(self.currently_inside_ut.is_none());
        }

        step
    }

    // TODO Maybe need to amend uber_turns?
    pub fn add(&mut self, step: PathStep, map: &Map) {
        self.total_length += step.as_traversable().length(map);
        match step {
            PathStep::Lane(_) | PathStep::ContraflowLane(_) => self.total_lanes += 1,
            _ => {}
        };
        self.steps.push_back(step);
    }

    // TODO This is a brittle, tied to exactly what opportunistically_lanechange does.
    pub fn approaching_uber_turn(&self) -> bool {
        if self.steps.len() < 5 || self.uber_turns.is_empty() {
            return false;
        }
        if let PathStep::Turn(t) = self.steps[1] {
            if self.uber_turns[0].path[0] == t {
                return true;
            }
        }
        if let PathStep::Turn(t) = self.steps[3] {
            if self.uber_turns[0].path[0] == t {
                return true;
            }
        }
        false
    }

    /// Trusting the caller to do this in valid ways.
    pub fn modify_step(&mut self, idx: usize, step: PathStep, map: &Map) {
        assert!(self.currently_inside_ut.is_none());
        assert!(idx != 0);
        self.total_length -= self.steps[idx].as_traversable().length(map);
        self.steps[idx] = step;
        self.total_length += self.steps[idx].as_traversable().length(map);

        if self.total_length < Distance::ZERO {
            panic!(
                "modify_step broke total_length, it's now {}",
                self.total_length
            );
        }
    }

    pub fn current_step(&self) -> PathStep {
        self.steps[0]
    }

    pub fn next_step(&self) -> PathStep {
        self.steps[1]
    }
    pub fn maybe_next_step(&self) -> Option<PathStep> {
        if self.is_last_step() {
            None
        } else {
            Some(self.next_step())
        }
    }

    pub fn last_step(&self) -> PathStep {
        self.steps[self.steps.len() - 1]
    }

    /// dist_ahead is unlimited when None.
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
        if let Ok((pts, dist)) = self.steps[0].slice(map, start_dist, dist_remaining) {
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
                    // We know there's at least some geometry if we made it here, so unwrap to
                    // verify that understanding.
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
            if let Ok((new_pts, dist)) =
                self.steps[i].slice(map, start_dist_this_step, dist_remaining)
            {
                if pts_so_far.is_some() {
                    match pts_so_far.unwrap().extend(new_pts) {
                        Ok(new) => {
                            pts_so_far = Some(new);
                        }
                        Err(err) => {
                            println!("WARNING: Couldn't trace some path: {}", err);
                            return None;
                        }
                    }
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

    // Not for walking paths
    fn append(&mut self, other: Path, map: &Map) {
        assert!(self.currently_inside_ut.is_none());
        assert!(other.currently_inside_ut.is_none());
        let turn = match (*self.steps.back().unwrap(), other.steps[0]) {
            (PathStep::Lane(src), PathStep::Lane(dst)) => TurnID {
                parent: map.get_l(src).dst_i,
                src,
                dst,
            },
            _ => unreachable!(),
        };
        self.steps.push_back(PathStep::Turn(turn));
        self.total_length += map.get_t(turn).geom.length();
        self.steps.extend(other.steps);
        self.total_length += other.total_length;
        self.total_lanes += other.total_lanes;
        self.uber_turns.extend(other.uber_turns);
    }
}

/// Who's asking for a path?
// TODO This is an awful name.
#[derive(Debug, Serialize, Deserialize, PartialOrd, Ord, EnumSetType)]
pub enum PathConstraints {
    Pedestrian,
    Car,
    Bike,
    Bus,
    Train,
}

impl PathConstraints {
    pub fn all() -> Vec<PathConstraints> {
        vec![
            PathConstraints::Pedestrian,
            PathConstraints::Car,
            PathConstraints::Bike,
            PathConstraints::Bus,
            PathConstraints::Train,
        ]
    }

    /// Not bijective, but this is the best guess of user intent
    pub fn from_lt(lt: LaneType) -> PathConstraints {
        match lt {
            LaneType::Sidewalk | LaneType::Shoulder => PathConstraints::Pedestrian,
            LaneType::Driving => PathConstraints::Car,
            LaneType::Biking => PathConstraints::Bike,
            LaneType::Bus => PathConstraints::Bus,
            LaneType::LightRail => PathConstraints::Train,
            _ => panic!("PathConstraints::from_lt({:?}) doesn't make sense", lt),
        }
    }

    // TODO Handle private zones here?
    pub fn can_use(self, l: &Lane, map: &Map) -> bool {
        match self {
            PathConstraints::Pedestrian => l.is_walkable(),
            PathConstraints::Car => l.is_driving(),
            PathConstraints::Bike => {
                if l.is_biking() {
                    true
                } else if l.is_driving() || (l.is_bus() && map.config.bikes_can_use_bus_lanes) {
                    let road = map.get_r(l.parent);
                    !road.osm_tags.is("bicycle", "no")
                        && !road
                            .osm_tags
                            .is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
                } else {
                    false
                }
            }
            PathConstraints::Bus => l.is_driving() || l.is_bus(),
            PathConstraints::Train => l.is_light_rail(),
        }
    }

    /// Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub(crate) fn filter_lanes(self, mut choices: Vec<LaneID>, map: &Map) -> Vec<LaneID> {
        choices.retain(|l| self.can_use(map.get_l(*l), map));
        if self == PathConstraints::Bike {
            let just_bike_lanes: Vec<LaneID> = choices
                .iter()
                .copied()
                .filter(|l| map.get_l(*l).is_biking())
                .collect();
            if !just_bike_lanes.is_empty() {
                return just_bike_lanes;
            }
        }
        choices
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PathRequest {
    pub start: Position,
    pub end: Position,
    pub constraints: PathConstraints,
}

impl fmt::Display for PathRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PathRequest({} along {}... to {} along {} for {:?})",
            self.start.dist_along(),
            self.start.lane(),
            self.end.dist_along(),
            self.end.lane(),
            self.constraints,
        )
    }
}

fn validate_continuity(map: &Map, steps: &Vec<PathStep>) {
    if steps.is_empty() {
        panic!("Empty path");
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
        if len > EPSILON_DIST {
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

fn validate_restrictions(map: &Map, steps: &Vec<PathStep>) {
    for triple in steps.windows(5) {
        if let (PathStep::Lane(l1), PathStep::Lane(l2), PathStep::Lane(l3)) =
            (triple[0], triple[2], triple[4])
        {
            let from = map.get_parent(l1);
            let via = map.get_l(l2).parent;
            let to = map.get_l(l3).parent;

            for (dont_via, dont_to) in &from.complicated_turn_restrictions {
                if via == *dont_via && to == *dont_to {
                    panic!(
                        "Some path does illegal uber-turn: {} -> {} -> {}",
                        l1, l2, l3
                    );
                }
            }
        }
    }
}

/// Most of the time, prefer using the faster contraction hierarchies. But sometimes, callers can
/// explicitly opt into a slower (but preparation-free) pathfinder that just uses Dijkstra's
/// maneuever.
#[derive(Serialize, Deserialize)]
pub enum Pathfinder {
    Dijkstra,
    CH(ContractionHierarchyPathfinder),
}

impl Pathfinder {
    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        match self {
            Pathfinder::Dijkstra => dijkstra::pathfind(req, map),
            Pathfinder::CH(ref p) => p.pathfind(req, map),
        }
    }
    pub fn pathfind_avoiding_lanes(
        &self,
        req: PathRequest,
        avoid: BTreeSet<LaneID>,
        map: &Map,
    ) -> Option<Path> {
        dijkstra::pathfind_avoiding_lanes(req, avoid, map)
    }

    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        match self {
            // TODO Implement this
            Pathfinder::Dijkstra => None,
            Pathfinder::CH(ref p) => p.should_use_transit(map, start, end),
        }
    }

    pub fn apply_edits(&mut self, map: &Map, timer: &mut Timer) {
        match self {
            Pathfinder::Dijkstra => {}
            Pathfinder::CH(ref mut p) => p.apply_edits(map, timer),
        }
    }
}
