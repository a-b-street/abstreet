mod driving;
mod node_map;
// TODO tmp
pub mod uber_turns;
mod walking;

pub use self::driving::cost;
use self::driving::VehiclePathfinder;
use self::walking::{one_step_walking_path, walking_path_to_steps, SidewalkPathfinder};
pub use self::walking::{walking_cost, WalkingNode};
use crate::{
    osm, BusRouteID, BusStopID, Intersection, Lane, LaneID, LaneType, Map, Position, Traversable,
    TurnID, UberTurn, Zone,
};
use abstutil::Timer;
use enumset::EnumSetType;
use geom::{Distance, PolyLine, EPSILON_DIST};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
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

    // Only used for weird serialization magic.
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
    pub fn lanes_crossed_so_far(&self) -> usize {
        let mut remaining = 0;
        for s in &self.steps {
            match s {
                PathStep::Lane(_) | PathStep::ContraflowLane(_) => remaining += 1,
                _ => {}
            };
        }
        self.total_lanes - remaining
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

    // Trusting the caller to do this in valid ways.
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
            if let Some((new_pts, dist)) =
                self.steps[i].slice(map, start_dist_this_step, dist_remaining)
            {
                if pts_so_far.is_some() {
                    if let Some(new) = pts_so_far.unwrap().maybe_extend(new_pts) {
                        pts_so_far = Some(new);
                    } else {
                        println!("WARNING: Couldn't trace some path because of duplicate points");
                        return None;
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

// Who's asking for a path?
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
    // Not bijective, but this is the best guess of user intent
    pub fn from_lt(lt: LaneType) -> PathConstraints {
        match lt {
            LaneType::Sidewalk => PathConstraints::Pedestrian,
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
            PathConstraints::Pedestrian => l.is_sidewalk(),
            PathConstraints::Car => l.is_driving(),
            PathConstraints::Bike => {
                if l.is_biking() {
                    true
                } else if l.is_driving() || (l.is_bus() && map.config.bikes_can_use_bus_lanes) {
                    let road = map.get_r(l.parent);
                    road.osm_tags.get("bicycle") != Some(&"no".to_string())
                        && road.osm_tags.get(osm::HIGHWAY) != Some(&"motorway".to_string())
                        && road.osm_tags.get(osm::HIGHWAY) != Some(&"motorway_link".to_string())
                } else {
                    false
                }
            }
            PathConstraints::Bus => l.is_driving() || l.is_bus(),
            PathConstraints::Train => l.is_light_rail(),
        }
    }

    // Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub fn filter_lanes(self, lanes: impl Iterator<Item = LaneID>, map: &Map) -> Vec<LaneID> {
        let choices: Vec<LaneID> = lanes.filter(|l| self.can_use(map.get_l(*l), map)).collect();
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

#[derive(Serialize, Deserialize)]
pub struct Pathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    train_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    // TODO Option just during initialization! Ewww.
    walking_with_transit_graph: Option<SidewalkPathfinder>,
}

impl Pathfinder {
    pub fn new_without_transit(map: &Map, timer: &mut Timer) -> Pathfinder {
        timer.start("prepare pathfinding for cars");
        let car_graph = VehiclePathfinder::new(map, PathConstraints::Car, None);
        timer.stop("prepare pathfinding for cars");

        // The edge weights for bikes are so different from the driving graph that reusing the node
        // ordering actually hurts!
        timer.start("prepare pathfinding for bikes");
        let bike_graph = VehiclePathfinder::new(map, PathConstraints::Bike, None);
        timer.stop("prepare pathfinding for bikes");

        timer.start("prepare pathfinding for buses");
        let bus_graph = VehiclePathfinder::new(map, PathConstraints::Bus, Some(&car_graph));
        timer.stop("prepare pathfinding for buses");

        timer.start("prepare pathfinding for trains");
        let train_graph = VehiclePathfinder::new(map, PathConstraints::Train, None);
        timer.stop("prepare pathfinding for trains");

        timer.start("prepare pathfinding for pedestrians");
        let walking_graph = SidewalkPathfinder::new(map, false, &bus_graph);
        timer.stop("prepare pathfinding for pedestrians");

        Pathfinder {
            car_graph,
            bike_graph,
            bus_graph,
            train_graph,
            walking_graph,
            walking_with_transit_graph: None,
        }
    }

    pub fn setup_walking_with_transit(&mut self, map: &Map) {
        self.walking_with_transit_graph = Some(SidewalkPathfinder::new(map, true, &self.bus_graph));
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        if req.start.lane() == req.end.lane() && req.constraints == PathConstraints::Pedestrian {
            return Some(one_step_walking_path(&req, map));
        }

        // If we start or end in a private zone, have to stitch together a smaller path with a path
        // through the main map.
        let start_r = map.get_parent(req.start.lane());
        let end_r = map.get_parent(req.end.lane());

        match (start_r.get_zone(map), end_r.get_zone(map)) {
            (Some(z1), Some(z2)) => {
                if z1 == z2 {
                    if !z1.allow_through_traffic.contains(req.constraints) {
                        if req.constraints == PathConstraints::Pedestrian {
                            let steps =
                                walking_path_to_steps(z1.pathfind_walking(req.clone(), map)?, map);
                            return Some(Path::new(map, steps, req.end.dist_along(), Vec::new()));
                        }
                        return z1.pathfind(req, map);
                    }
                } else {
                    // TODO Handle paths going between two different zones
                    return None;
                }
            }
            (Some(zone), None) => {
                if !zone.allow_through_traffic.contains(req.constraints) {
                    let mut borders: Vec<&Intersection> =
                        zone.borders.iter().map(|i| map.get_i(*i)).collect();
                    // TODO Use the CH to pick the lowest overall cost?
                    let pt = req.end.pt(map);
                    borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

                    for i in borders {
                        if let Some(result) = self.pathfind_from_zone(i, req.clone(), zone, map) {
                            return Some(result);
                        }
                    }
                    return None;
                }
            }
            (None, Some(zone)) => {
                if !zone.allow_through_traffic.contains(req.constraints) {
                    let mut borders: Vec<&Intersection> =
                        zone.borders.iter().map(|i| map.get_i(*i)).collect();
                    // TODO Use the CH to pick the lowest overall cost?
                    let pt = req.start.pt(map);
                    borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

                    for i in borders {
                        if let Some(result) = self.pathfind_to_zone(i, req.clone(), zone, map) {
                            return Some(result);
                        }
                    }
                    return None;
                }
            }
            (None, None) => {}
        }
        match req.constraints {
            PathConstraints::Pedestrian => {
                let steps = walking_path_to_steps(self.walking_graph.pathfind(&req, map)?, map);
                Some(Path::new(map, steps, req.end.dist_along(), Vec::new()))
            }
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Train => self.train_graph.pathfind(&req, map).map(|(p, _)| p),
        }
    }

    // TODO Alright, reconsider refactoring pieces of this again. :)
    fn pathfind_from_zone(
        &self,
        i: &Intersection,
        mut req: PathRequest,
        zone: &Zone,
        map: &Map,
    ) -> Option<Path> {
        // Because sidewalks aren't all immediately linked, insist on a (src, dst) combo that
        // are actually connected by a turn.
        let src_choices = i
            .get_incoming_lanes(map, req.constraints)
            .filter(|l| zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let dst_choices = i
            .get_outgoing_lanes(map, req.constraints)
            .into_iter()
            .filter(|l| !zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let (src, dst) = {
            let mut result = None;
            'OUTER: for l1 in src_choices {
                for l2 in &dst_choices {
                    if l1 != *l2
                        && map
                            .maybe_get_t(TurnID {
                                parent: i.id,
                                src: l1,
                                dst: *l2,
                            })
                            .is_some()
                    {
                        result = Some((l1, *l2));
                        break 'OUTER;
                    }
                }
            }
            result?
        };

        let interior_req = PathRequest {
            start: req.start,
            end: if map.get_l(src).dst_i == i.id {
                Position::end(src, map)
            } else {
                Position::start(src)
            },
            constraints: req.constraints,
        };
        req.start = if map.get_l(dst).src_i == i.id {
            Position::start(dst)
        } else {
            Position::end(dst, map)
        };

        if let PathConstraints::Pedestrian = req.constraints {
            let mut interior_path = zone.pathfind_walking(interior_req, map)?;
            let main_path = if req.start.lane() == req.end.lane() {
                let mut one_step = vec![
                    WalkingNode::closest(req.start, map),
                    WalkingNode::closest(req.end, map),
                ];
                one_step.dedup();
                one_step
            } else {
                self.walking_graph.pathfind(&req, map)?
            };
            interior_path.extend(main_path);
            let steps = walking_path_to_steps(interior_path, map);
            return Some(Path::new(map, steps, req.end.dist_along(), Vec::new()));
        }

        let mut interior_path = zone.pathfind(interior_req, map)?;
        let main_path = match req.constraints {
            PathConstraints::Pedestrian => unreachable!(),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Train => self.train_graph.pathfind(&req, map).map(|(p, _)| p),
        }?;
        interior_path.append(main_path, map);
        Some(interior_path)
    }

    fn pathfind_to_zone(
        &self,
        i: &Intersection,
        mut req: PathRequest,
        zone: &Zone,
        map: &Map,
    ) -> Option<Path> {
        // Because sidewalks aren't all immediately linked, insist on a (src, dst) combo that
        // are actually connected by a turn.
        let src_choices = i
            .get_incoming_lanes(map, req.constraints)
            .filter(|l| !zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let dst_choices = i
            .get_outgoing_lanes(map, req.constraints)
            .into_iter()
            .filter(|l| zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let (src, dst) = {
            let mut result = None;
            'OUTER: for l1 in src_choices {
                for l2 in &dst_choices {
                    if l1 != *l2
                        && map
                            .maybe_get_t(TurnID {
                                parent: i.id,
                                src: l1,
                                dst: *l2,
                            })
                            .is_some()
                    {
                        result = Some((l1, *l2));
                        break 'OUTER;
                    }
                }
            }
            result?
        };

        let interior_req = PathRequest {
            start: if map.get_l(dst).src_i == i.id {
                Position::start(dst)
            } else {
                Position::end(dst, map)
            },
            end: req.end,
            constraints: req.constraints,
        };
        let orig_end_dist = req.end.dist_along();
        req.end = if map.get_l(src).dst_i == i.id {
            Position::end(src, map)
        } else {
            Position::start(src)
        };

        if let PathConstraints::Pedestrian = req.constraints {
            let interior_path = zone.pathfind_walking(interior_req, map)?;
            let mut main_path = if req.start.lane() == req.end.lane() {
                let mut one_step = vec![
                    WalkingNode::closest(req.start, map),
                    WalkingNode::closest(req.end, map),
                ];
                one_step.dedup();
                one_step
            } else {
                self.walking_graph.pathfind(&req, map)?
            };

            main_path.extend(interior_path);
            let steps = walking_path_to_steps(main_path, map);
            return Some(Path::new(map, steps, orig_end_dist, Vec::new()));
        }

        let interior_path = zone.pathfind(interior_req, map)?;
        let mut main_path = match req.constraints {
            PathConstraints::Pedestrian => unreachable!(),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Train => self.train_graph.pathfind(&req, map).map(|(p, _)| p),
        }?;
        main_path.append(interior_path, map);
        main_path.end_dist = orig_end_dist;
        Some(main_path)
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

    pub fn apply_edits(&mut self, map: &Map, timer: &mut Timer) {
        timer.start("apply edits to car pathfinding");
        self.car_graph.apply_edits(map);
        timer.stop("apply edits to car pathfinding");

        timer.start("apply edits to bike pathfinding");
        self.bike_graph.apply_edits(map);
        timer.stop("apply edits to bike pathfinding");

        timer.start("apply edits to bus pathfinding");
        self.bus_graph.apply_edits(map);
        timer.stop("apply edits to bus pathfinding");

        // Can't edit anything related to trains

        timer.start("apply edits to pedestrian pathfinding");
        self.walking_graph.apply_edits(map, &self.bus_graph);
        timer.stop("apply edits to pedestrian pathfinding");

        timer.start("apply edits to pedestrian using transit pathfinding");
        self.walking_with_transit_graph
            .as_mut()
            .unwrap()
            .apply_edits(map, &self.bus_graph);
        timer.stop("apply edits to pedestrian using transit pathfinding");
    }
}
