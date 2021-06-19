use std::collections::VecDeque;
use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use geom::{Distance, Duration, PolyLine, Speed, EPSILON_DIST};

use crate::{BuildingID, LaneID, Map, PathConstraints, Position, Traversable, TurnID, UberTurn};

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

    // start is relative to the start of the actual geometry -- so from the lane's real start for
    // ContraflowLane.
    fn exact_slice(
        &self,
        map: &Map,
        start: Distance,
        dist_ahead: Option<Distance>,
    ) -> Result<PolyLine> {
        if let Some(d) = dist_ahead {
            if d < Distance::ZERO {
                panic!("Negative dist_ahead?! {}", d);
            }
            if d == Distance::ZERO {
                bail!("0 dist ahead for slice");
            }
        }

        match self {
            PathStep::Lane(id) => {
                let pts = &map.get_l(*id).lane_center_pts;
                if let Some(d) = dist_ahead {
                    pts.maybe_exact_slice(start, start + d)
                } else {
                    pts.maybe_exact_slice(start, pts.length())
                }
            }
            PathStep::ContraflowLane(id) => {
                let pts = map.get_l(*id).lane_center_pts.reversed();
                let reversed_start = pts.length() - start;
                if let Some(d) = dist_ahead {
                    pts.maybe_exact_slice(reversed_start, reversed_start + d)
                } else {
                    pts.maybe_exact_slice(reversed_start, pts.length())
                }
            }
            PathStep::Turn(id) => {
                let pts = &map.get_t(*id).geom;
                if let Some(d) = dist_ahead {
                    pts.maybe_exact_slice(start, start + d)
                } else {
                    pts.maybe_exact_slice(start, pts.length())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Path {
    steps: VecDeque<PathStep>,
    // The original request used to produce this path. Calling shift(), add(), modify_step(), etc
    // will NOT affect this.
    orig_req: PathRequest,

    // Also track progress along the original path.
    total_length: Distance,
    crossed_so_far: Distance,

    // A list of uber-turns encountered by this path, in order. The steps are flattened into the
    // sequence of turn->lane->...->turn.
    uber_turns: VecDeque<UberTurn>,
    // Is the current_step in the middle of an UberTurn?
    currently_inside_ut: Option<UberTurn>,

    blocked_starts: Vec<Position>,
}

impl Path {
    pub(crate) fn new(
        map: &Map,
        steps: Vec<PathStep>,
        orig_req: PathRequest,
        uber_turns: Vec<UberTurn>,
        blocked_starts: Vec<Position>,
    ) -> Path {
        // Haven't seen problems here in a very long time. Noticeably saves some time to skip.
        if false {
            validate_continuity(map, &steps);
        }
        if false {
            validate_restrictions(map, &steps);
        }
        if false {
            validate_zones(map, &steps, &orig_req);
        }
        let mut path = Path {
            steps: VecDeque::from(steps),
            orig_req,
            total_length: Distance::ZERO,
            crossed_so_far: Distance::ZERO,
            uber_turns: uber_turns.into_iter().collect(),
            currently_inside_ut: None,
            blocked_starts,
        };
        for step in &path.steps {
            path.total_length += path.dist_crossed_from_step(map, step);
        }
        path
    }

    /// Once we finish this PathStep, how much distance will be crossed? If the step is at the
    /// beginning or end of our path, then the full length may not be used.
    pub fn dist_crossed_from_step(&self, map: &Map, step: &PathStep) -> Distance {
        match step {
            PathStep::Lane(l) => {
                let lane = map.get_l(*l);
                if self.orig_req.start.lane() == lane.id {
                    lane.length() - self.orig_req.start.dist_along()
                } else if self.orig_req.end.lane() == lane.id {
                    self.orig_req.end.dist_along()
                } else {
                    lane.length()
                }
            }
            PathStep::ContraflowLane(l) => {
                let lane = map.get_l(*l);
                if self.orig_req.start.lane() == lane.id {
                    self.orig_req.start.dist_along()
                } else if self.orig_req.end.lane() == lane.id {
                    lane.length() - self.orig_req.end.dist_along()
                } else {
                    lane.length()
                }
            }
            PathStep::Turn(t) => map.get_t(*t).geom.length(),
        }
    }

    pub fn one_step(req: PathRequest, map: &Map) -> Path {
        assert_eq!(req.start.lane(), req.end.lane());
        Path::new(
            map,
            vec![PathStep::Lane(req.start.lane())],
            req,
            Vec::new(),
            Vec::new(),
        )
    }

    /// The original PathRequest used to produce this path. If the path has been modified since
    /// creation, the start and end of the request won't match up with the current path steps.
    pub fn get_req(&self) -> &PathRequest {
        &self.orig_req
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
        self.crossed_so_far += self.dist_crossed_from_step(map, &step);

        #[allow(clippy::collapsible_if)] // better readability
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

    pub fn add(&mut self, step: PathStep, map: &Map) {
        if let Some(PathStep::Lane(l)) = self.steps.back() {
            if *l == self.orig_req.end.lane() {
                self.total_length += map.get_l(*l).length() - self.orig_req.end.dist_along();
            }
        }
        // TODO We assume we'll be going along the full length of this new step
        self.total_length += step.as_traversable().get_polyline(map).length();

        self.steps.push_back(step);
        // TODO Maybe need to amend uber_turns?
    }

    pub fn is_upcoming_uber_turn_component(&self, t: TurnID) -> bool {
        self.uber_turns
            .front()
            .map(|ut| ut.path.contains(&t))
            .unwrap_or(false)
    }

    /// Trusting the caller to do this in valid ways.
    pub fn modify_step(&mut self, idx: usize, step: PathStep, map: &Map) {
        assert!(self.currently_inside_ut.is_none());
        // We're assuming this step was in the middle of the path, meaning we were planning to
        // travel its full length
        self.total_length -= self.steps[idx].as_traversable().get_polyline(map).length();

        // When replacing a turn, also update any references to it in uber_turns
        if let PathStep::Turn(old_turn) = self.steps[idx] {
            for uts in &mut self.uber_turns {
                if let Some(turn_idx) = uts.path.iter().position(|i| i == &old_turn) {
                    if let PathStep::Turn(new_turn) = step {
                        uts.path[turn_idx] = new_turn;
                    } else {
                        panic!("expected turn, but found {:?}", step);
                    }
                }
            }
        }

        self.steps[idx] = step;
        self.total_length += self.steps[idx].as_traversable().get_polyline(map).length();

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

    /// Traces along the path from its originally requested start. This is only valid to call for
    /// an umodified path.
    pub fn trace(&self, map: &Map) -> Option<PolyLine> {
        let t1 = self.steps[0].as_traversable();
        let t2 = Traversable::Lane(self.orig_req.start.lane());
        if t1 != t2 {
            warn!(
                "Can't trace modified path; first step is {}, but requested started from {}",
                t1, t2
            );
            return None;
        }
        self.trace_from_start(map, self.orig_req.start.dist_along())
    }

    /// Traces along the path from a specified distance along the first step until the end.
    pub fn trace_from_start(&self, map: &Map, start_dist: Distance) -> Option<PolyLine> {
        let orig_end_dist = self.orig_req.end.dist_along();

        if self.steps.len() == 1 {
            let dist_ahead = if start_dist < orig_end_dist {
                orig_end_dist - start_dist
            } else {
                start_dist - orig_end_dist
            };

            // Why might this fail? It's possible there are paths on their last step that're
            // effectively empty, because they're a 0-length turn, or something like a pedestrian
            // crossing a front path and immediately getting on a bike.
            return self.steps[0]
                .exact_slice(map, start_dist, Some(dist_ahead))
                .ok();
        }

        let mut pts_so_far: Option<PolyLine> = None;

        // Special case the first step with start_dist.
        if let Ok(pts) = self.steps[0].exact_slice(map, start_dist, None) {
            pts_so_far = Some(pts);
        }

        // Crunch through the intermediate steps, as long as we can.
        for i in 1..self.steps.len() {
            // Restrict the last step's slice
            let dist_ahead = if i == self.steps.len() - 1 {
                Some(match self.steps[i] {
                    PathStep::ContraflowLane(l) => {
                        map.get_l(l).lane_center_pts.reversed().length() - orig_end_dist
                    }
                    _ => orig_end_dist,
                })
            } else {
                None
            };

            let start_dist_this_step = match self.steps[i] {
                // TODO Length of a PolyLine can slightly change when points are reversed! That
                // seems bad.
                PathStep::ContraflowLane(l) => map.get_l(l).lane_center_pts.reversed().length(),
                _ => Distance::ZERO,
            };
            if let Ok(new_pts) = self.steps[i].exact_slice(map, start_dist_this_step, dist_ahead) {
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
            }
        }

        Some(pts_so_far.unwrap())
    }

    pub fn get_steps(&self) -> &VecDeque<PathStep> {
        &self.steps
    }

    /// Estimate how long following the path will take in the best case, assuming no traffic or
    /// delay at intersections. To determine the speed along each step, the agent following their
    /// path and their optional max_speed must be specified.
    pub fn estimate_duration(
        &self,
        map: &Map,
        constraints: PathConstraints,
        max_speed: Option<Speed>,
    ) -> Duration {
        let mut total = Duration::ZERO;
        for step in &self.steps {
            let dist = self.dist_crossed_from_step(map, step);
            let speed = step
                .as_traversable()
                .max_speed_along(max_speed, constraints, map);
            total += dist / speed;
        }
        total
    }

    /// If the agent following this path will initially block some intermediate lanes as they move
    /// between a driveway and `get_req().start`, then record those equivalent positions here.
    pub fn get_blocked_starts(&self) -> &Vec<Position> {
        &self.blocked_starts
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

impl PathRequest {
    /// Determines the start and end position to travel between two buildings for a certain mode.
    /// The path won't cover modality transfers -- if somebody has to walk between the building and
    /// a parking spot or bikeable position, that won't be captured here.
    pub fn between_buildings(
        map: &Map,
        from: BuildingID,
        to: BuildingID,
        constraints: PathConstraints,
    ) -> Option<PathRequest> {
        let from = map.get_b(from);
        let to = map.get_b(to);
        let (start, end) = match constraints {
            PathConstraints::Pedestrian => (from.sidewalk_pos, to.sidewalk_pos),
            PathConstraints::Bike => (from.biking_connection(map)?.0, to.biking_connection(map)?.0),
            PathConstraints::Car => (
                from.driving_connection(map)?.0,
                to.driving_connection(map)?.0,
            ),
            // These two aren't useful here. A pedestrian using transit would pass in
            // PathConstraints::Pedestrian. There's no reason yet to find a route for a bus or
            // train to travel between buildings.
            PathConstraints::Bus | PathConstraints::Train => unimplemented!(),
        };
        Some(PathRequest {
            start,
            end,
            constraints,
        })
    }
}

fn validate_continuity(map: &Map, steps: &[PathStep]) {
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

fn validate_restrictions(map: &Map, steps: &[PathStep]) {
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

fn validate_zones(map: &Map, steps: &[PathStep], req: &PathRequest) {
    let z1 = map.get_parent(req.start.lane()).get_zone(map);
    let z2 = map.get_parent(req.end.lane()).get_zone(map);

    for step in steps {
        if let PathStep::Turn(t) = step {
            if map
                .get_parent(t.src)
                .access_restrictions
                .allow_through_traffic
                .contains(req.constraints)
                && !map
                    .get_parent(t.dst)
                    .access_restrictions
                    .allow_through_traffic
                    .contains(req.constraints)
            {
                // Entering our destination zone is fine
                let into_zone = map.get_parent(t.dst).get_zone(map);
                if into_zone != z1 && into_zone != z2 {
                    // TODO There are lots of false positive here that occur when part of the graph
                    // is separated from the rest by access-restricted roads. Could maybe detect
                    // that here, or ideally even extend the zone at map construction time (or edit
                    // time) when that happens.
                    panic!("{} causes illegal entrance into a zone at {}", req, t);
                }
            }
        }
    }
}
