mod driving;
mod node_map;
// TODO tmp
pub mod uber_turns;
mod walking;

pub use self::driving::cost;
use self::driving::VehiclePathfinder;
use self::walking::SidewalkPathfinder;
pub use self::walking::{one_step_walking_path, walking_cost, walking_path_to_steps, WalkingNode};
use crate::{
    osm, BusRouteID, BusStopID, Intersection, Lane, LaneID, LaneType, Map, Position, Traversable,
    TurnID, Zone,
};
use abstutil::Timer;
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
}

impl Path {
    pub(crate) fn new(map: &Map, steps: Vec<PathStep>, end_dist: Distance) -> Path {
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

    pub fn is_last_step(&self) -> bool {
        self.steps.len() == 1
    }

    pub fn isnt_last_step(&self) -> bool {
        self.steps.len() > 1
    }

    pub fn shift(&mut self, map: &Map) -> PathStep {
        let step = self.steps.pop_front().unwrap();
        self.crossed_so_far += step.as_traversable().length(map);
        step
    }

    pub fn add(&mut self, step: PathStep, map: &Map) {
        self.total_length += step.as_traversable().length(map);
        match step {
            PathStep::Lane(_) | PathStep::ContraflowLane(_) => self.total_lanes += 1,
            _ => {}
        };
        self.steps.push_back(step);
    }

    // Trusting the caller to do this in valid ways.
    pub fn modify_step(&mut self, idx: usize, step: PathStep, map: &Map) {
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

    fn prepend(&mut self, other: Path, map: &Map) {
        let turn = glue(*other.steps.back().unwrap(), self.steps[0], map);
        self.steps.push_front(PathStep::Turn(turn));
        self.total_length += map.get_t(turn).geom.length();
        for step in other.steps.into_iter().rev() {
            self.steps.push_front(step);
        }
        self.total_length += other.total_length;
        self.total_lanes += other.total_lanes;
    }

    fn append(&mut self, other: Path, map: &Map) {
        let turn = glue(*self.steps.back().unwrap(), other.steps[0], map);
        self.steps.push_back(PathStep::Turn(turn));
        self.total_length += map.get_t(turn).geom.length();
        self.steps.extend(other.steps);
        self.total_length += other.total_length;
        self.total_lanes += other.total_lanes;
    }
}

fn glue(step1: PathStep, step2: PathStep, map: &Map) -> TurnID {
    match step1 {
        PathStep::Lane(src) => match step2 {
            PathStep::Lane(dst) | PathStep::ContraflowLane(dst) => TurnID {
                parent: map.get_l(src).dst_i,
                src,
                dst,
            },
            _ => unreachable!(),
        },
        PathStep::ContraflowLane(src) => match step2 {
            PathStep::Lane(dst) | PathStep::ContraflowLane(dst) => TurnID {
                parent: map.get_l(src).src_i,
                src,
                dst,
            },
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

// Who's asking for a path?
// TODO This is an awful name.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathConstraints {
    Pedestrian,
    Car,
    Bike,
    Bus,
}

impl PathConstraints {
    // Not bijective, but this is the best guess of user intent
    pub fn from_lt(lt: LaneType) -> PathConstraints {
        match lt {
            LaneType::Sidewalk => PathConstraints::Pedestrian,
            LaneType::Driving => PathConstraints::Car,
            LaneType::Biking => PathConstraints::Bike,
            LaneType::Bus => PathConstraints::Bus,
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
                } else if l.is_driving() || l.is_bus() {
                    // Note bikes can use bus lanes -- this is generally true in Seattle.
                    let road = map.get_r(l.parent);
                    road.osm_tags.get("bicycle") != Some(&"no".to_string())
                        && road.osm_tags.get(osm::HIGHWAY) != Some(&"motorway".to_string())
                        && road.osm_tags.get(osm::HIGHWAY) != Some(&"motorway_link".to_string())
                } else {
                    false
                }
            }
            PathConstraints::Bus => l.is_driving() || l.is_bus(),
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

        timer.start("prepare pathfinding for pedestrians");
        let walking_graph = SidewalkPathfinder::new(map, false, &bus_graph);
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
        self.walking_with_transit_graph = Some(SidewalkPathfinder::new(map, true, &self.bus_graph));
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        // If we start or end in a private zone, have to stitch together a smaller path with a path
        // through the main map.
        let start_r = map.get_parent(req.start.lane());
        let end_r = map.get_parent(req.end.lane());

        if start_r.zone.is_some() && end_r.zone.is_some() {
            if start_r.zone == end_r.zone {
                let zone = map.get_z(start_r.zone.unwrap());
                if !zone.allow_through_traffic.contains(&req.constraints) {
                    return zone.pathfind(req, map);
                }
            } else {
                // TODO Handle paths going between two different zones
                return None;
            }
        } else if let Some(z) = start_r.zone {
            let zone = map.get_z(z);
            if !zone.allow_through_traffic.contains(&req.constraints) {
                if req.constraints == PathConstraints::Pedestrian {
                    return None;
                }
                let mut borders: Vec<&Intersection> =
                    zone.borders.iter().map(|i| map.get_i(*i)).collect();
                // TODO Use the CH to pick the lowest overall cost?
                let pt = req.end.pt(map);
                borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

                for i in borders {
                    if let Some(result) = self.pathfind_from_zone(i, req.clone(), zone, map) {
                        validate_continuity(map, &result.steps.iter().cloned().collect());
                        return Some(result);
                    }
                }
                return None;
            }
        } else if let Some(z) = end_r.zone {
            let zone = map.get_z(z);
            if !zone.allow_through_traffic.contains(&req.constraints) {
                if req.constraints == PathConstraints::Pedestrian {
                    return None;
                }
                let mut borders: Vec<&Intersection> =
                    zone.borders.iter().map(|i| map.get_i(*i)).collect();
                // TODO Use the CH to pick the lowest overall cost?
                let pt = req.start.pt(map);
                borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

                for i in borders {
                    if let Some(result) = self.pathfind_to_zone(i, req.clone(), zone, map) {
                        validate_continuity(map, &result.steps.iter().cloned().collect());
                        return Some(result);
                    }
                }
                return None;
            }
        }

        match req.constraints {
            PathConstraints::Pedestrian => self.walking_graph.pathfind(&req, map),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
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

        let interior_path = zone.pathfind(
            PathRequest {
                start: req.start,
                end: if map.get_l(src).dst_i == i.id {
                    Position::end(src, map)
                } else {
                    Position::start(src)
                },
                constraints: req.constraints,
            },
            map,
        )?;
        req.start = match interior_path.steps.back().unwrap() {
            PathStep::Lane(_) => Position::start(dst),
            PathStep::ContraflowLane(_) => Position::end(dst, map),
            _ => unreachable!(),
        };
        let mut main_path = match req.constraints {
            PathConstraints::Pedestrian => self.walking_graph.pathfind(&req, map),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
        }?;
        main_path.prepend(interior_path, map);
        Some(main_path)
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

        let interior_path = zone.pathfind(
            PathRequest {
                start: if map.get_l(dst).src_i == i.id {
                    Position::start(dst)
                } else {
                    Position::end(dst, map)
                },
                end: req.end,
                constraints: req.constraints,
            },
            map,
        )?;
        let orig_end_dist = req.end.dist_along();
        req.end = match interior_path.steps[0] {
            PathStep::Lane(_) => Position::end(src, map),
            PathStep::ContraflowLane(_) => Position::start(src),
            _ => unreachable!(),
        };

        let mut main_path = match req.constraints {
            PathConstraints::Pedestrian => self.walking_graph.pathfind(&req, map),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
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
