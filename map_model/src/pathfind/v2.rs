//! Structures related to the new road-based pathfinding
//! (https://github.com/a-b-street/abstreet/issues/555) live here. When the transition is done,
//! things here will probably move into pathfind/mod.rs.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use geom::{Duration, Polygon, Ring, Speed};

use crate::pathfind::uber_turns::UberTurnV2;
use crate::{
    osm, DirectedRoadID, Direction, IntersectionID, LaneID, Map, MovementID, Path, PathConstraints,
    PathRequest, PathStep, RoadID, TurnID, UberTurn,
};

/// One step along a path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PathStepV2 {
    /// Original direction
    Along(DirectedRoadID),
    /// Opposite direction, sidewalks only
    Contraflow(DirectedRoadID),
    Movement(MovementID),
    ContraflowMovement(MovementID),
}

/// A path between two endpoints for a particular mode. This representation is immutable and doesn't
/// prescribe specific lanes and turns to follow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathV2 {
    steps: Vec<PathStepV2>,
    // TODO There will be a PathRequestV2, but I'm not sure how it'll change yet.
    req: PathRequest,
    cost: Duration,
    // TODO Temporarily we'll keep plumbing these along for path_v2_to_v1 to work, but we'll
    // probably just discover uber-turns lazily at the simulation layer instead.
    uber_turns: Vec<UberTurnV2>,

    // If alt_start was used, this may differ from req.start.lane()
    orig_start_lane: LaneID,
}

impl PathV2 {
    pub(crate) fn new(
        map: &Map,
        steps: Vec<PathStepV2>,
        mut req: PathRequest,
        cost: Duration,
        uber_turns: Vec<UberTurnV2>,
    ) -> PathV2 {
        let orig_start_lane = req.start.lane();

        // If we had two possible start positions, figure out which one we wound up using
        // TODO Doesn't make sense for pedestrians yet
        if req.constraints != PathConstraints::Pedestrian {
            if let Some((pos, _)) = req.alt_start {
                if let PathStepV2::Along(dr) = steps[0] {
                    if map.get_l(req.start.lane()).get_directed_parent() == dr {
                        // We used the original side, fine. No need to preserve this.
                    } else {
                        assert_eq!(map.get_l(pos.lane()).get_directed_parent(), dr);
                        req.start = pos;
                    }
                    req.alt_start = None;
                } else {
                    unreachable!()
                }
            }
        }

        // TODO Port validate_continuity and validate_restrictions?
        PathV2 {
            steps,
            req,
            cost,
            uber_turns,
            orig_start_lane,
        }
    }

    /// Vehicle implementations often just calculate the sequence of roads. Turn that into
    /// PathStepV2 here.
    pub fn from_roads(
        mut roads: Vec<DirectedRoadID>,
        req: PathRequest,
        cost: Duration,
        uber_turns: Vec<UberTurnV2>,
        map: &Map,
    ) -> PathV2 {
        let mut steps = Vec::new();
        for pair in roads.windows(2) {
            steps.push(PathStepV2::Along(pair[0]));
            steps.push(PathStepV2::Movement(MovementID {
                from: pair[0],
                to: pair[1],
                parent: pair[0].dst_i(map),
                crosswalk: false,
            }));
        }
        steps.push(PathStepV2::Along(roads.pop().unwrap()));
        PathV2::new(map, steps, req, cost, uber_turns)
    }

    /// The original PathRequest used to produce this path.
    pub fn get_req(&self) -> &PathRequest {
        &self.req
    }

    /// All steps in this path.
    pub fn get_steps(&self) -> &Vec<PathStepV2> {
        &self.steps
    }

    /// The time needed to perform this path. This time is not a lower bound; physically following
    /// the path might be faster. This time incorporates costs like using sub-optimal lanes, taking
    /// difficult turns, and crossing private roads (which are modelled with a large penalty!)
    pub fn get_cost(&self) -> Duration {
        self.cost
    }

    /// Estimate how long following the path will take in the best case, assuming no traffic, delay
    /// at intersections, elevation, or penalties for crossing private roads. To determine the
    /// speed along each step, the agent's optional max_speed must be known.
    ///
    /// TODO Hack. The one use of this actually needs to apply main_road_penalty. We want to omit
    /// some penalties, but use others. Come up with a better way of expressing this.
    pub fn estimate_duration(
        &self,
        map: &Map,
        max_speed: Option<Speed>,
        main_road_penalty: Option<f64>,
    ) -> Duration {
        let mut total = Duration::ZERO;
        for step in &self.steps {
            let (dist, mut speed);
            let mut multiplier = 1.0;
            match step {
                PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                    let road = map.get_r(dr.road);
                    dist = road.length();
                    speed = road.speed_limit;

                    if let Some(penalty) = main_road_penalty {
                        if road.get_rank() != osm::RoadRank::Local {
                            multiplier = penalty;
                        }
                    }
                }
                PathStepV2::Movement(m) | PathStepV2::ContraflowMovement(m) => {
                    if let Some(movement) = map.get_movement(*m) {
                        dist = movement.geom.length();
                        speed = map
                            .get_r(m.from.road)
                            .speed_limit
                            .min(map.get_r(m.to.road).speed_limit);
                    } else {
                        // Assume it's a SharedSidewalkCorner and just skip
                        continue;
                    }
                }
            }
            if let Some(max) = max_speed {
                speed = speed.min(max);
            }

            total += multiplier * (dist / speed);
        }
        total
    }

    /// Transform a sequence of roads representing a path into the current lane-based path, by
    /// picking particular lanes and turns to use.
    pub fn into_v1(mut self, map: &Map) -> Result<Path> {
        if self.req.constraints == PathConstraints::Pedestrian {
            return self.into_v1_walking(map);
        }

        // This is a somewhat brute-force method: run Dijkstra's algorithm on a graph of lanes and
        // turns, but only build the graph along the path of roads we've already found. This handles
        // arbitrary lookahead needed, and forces use of the original start/end lanes requested.
        let mut graph = petgraph::graphmap::DiGraphMap::new();
        for step in &self.steps {
            if let PathStepV2::Movement(mvmnt) = step {
                for src in mvmnt.from.lanes(self.req.constraints, map) {
                    for dst in mvmnt.to.lanes(self.req.constraints, map) {
                        let turn = TurnID {
                            parent: map.get_l(src).dst_i,
                            src,
                            dst,
                        };
                        if map.maybe_get_t(turn).is_some() {
                            graph.add_edge(src, dst, turn);
                        }
                    }
                }
            }
        }

        // The v2 path might immediately require a turn that's only available from some lanes. If
        // the req.start lane can't make that turn, then producing the v1 path would fail. So let's
        // allow starting from any lane on the same side of the road. Since petgraph can only start
        // from a single node and since we want to prefer the originally requested lane anyway,
        // create a virtual start node and connect it to all possible starting lanes.
        let virtual_start_node = LaneID {
            road: RoadID(map.all_roads().len()),
            offset: 0,
        };
        let start_lane = self.req.start.lane();
        let start_road = map.get_parent(start_lane);
        let start_lane_idx = start_lane.offset as isize;
        for l in map
            .get_l(start_lane)
            .get_directed_parent()
            .lanes(self.req.constraints, map)
        {
            // Heavily penalize starting from something other than the originally requested lane.
            // At the simulation layer, we may need to block intermediate lanes to exit a driveway,
            // so reflect that cost here. The high cost should only be worth it when the v2 path
            // requires that up-front turn from certain lanes.
            //
            // TODO This is only valid if we were leaving from a driveway! This is making some
            // buses warp after making a stop.
            let idx_dist = (start_lane_idx - (l.offset as isize)).abs();
            let cost = 100 * idx_dist as usize;
            let fake_turn = TurnID {
                // Just encode the cost here for convenience
                parent: IntersectionID(cost),
                src: virtual_start_node,
                dst: virtual_start_node,
            };
            graph.add_edge(virtual_start_node, l, fake_turn);
        }

        match petgraph::algo::astar(
            &graph,
            virtual_start_node,
            |l| l == self.req.end.lane(),
            |(_, _, t)| {
                if t.src == virtual_start_node {
                    return t.parent.0;
                }

                // Normally opportunistic lane-changing adjusts the path live, but that doesn't work
                // near uber-turns. So still use some of the penalties here.
                let (lt, lc, slow_lane) = map.get_t(*t).penalty(self.req.constraints, map);
                let mut extra_penalty = lt + lc;
                if self.req.constraints == PathConstraints::Bike {
                    extra_penalty += slow_lane;
                }
                // Always treat every lane/turn as at least cost 1; otherwise A* can't understand
                // that a final path with 10 steps costs more than one with 5. The
                // road-based pathfinding has already chosen the overall route; when
                // we're picking individual lanes, the length of each lane along one
                // road is going to be about the same.
                let base = 1;
                base + extra_penalty
            },
            |_| 0,
        ) {
            Some((_, path)) => {
                let mut steps = Vec::new();
                // Skip the first node; it's always virtual_start_node
                assert_eq!(path[0], virtual_start_node);
                for pair in path.windows(2) {
                    if pair[0] == virtual_start_node {
                        continue;
                    }

                    steps.push(PathStep::Lane(pair[0]));
                    // We don't need to look for this turn in the map; we know it exists.
                    steps.push(PathStep::Turn(TurnID {
                        parent: map.get_l(pair[0]).dst_i,
                        src: pair[0],
                        dst: pair[1],
                    }));
                }
                steps.push(PathStep::Lane(self.req.end.lane()));
                let mut blocked_starts = Vec::new();
                if steps[0] != PathStep::Lane(self.orig_start_lane) {
                    let actual_start = match steps[0] {
                        PathStep::Lane(l) => l,
                        _ => unreachable!(),
                    };
                    blocked_starts.push(self.orig_start_lane);
                    blocked_starts
                        .extend(start_road.get_lanes_between(self.orig_start_lane, actual_start));
                    // Sometimes a no-op for exiting off-side
                    self.req.start = self.req.start.equiv_pos(actual_start, map);
                }
                let uber_turns = find_uber_turns(&steps, map, self.uber_turns);
                Ok(Path::new(map, steps, self.req, uber_turns, blocked_starts))
            }
            None => bail!(
                "Can't transform a road-based path to a lane-based path for {}",
                self.req
            ),
        }
    }

    fn into_v1_walking(self, map: &Map) -> Result<Path> {
        let mut steps = Vec::new();
        for step in self.steps {
            steps.push(match step {
                PathStepV2::Along(r) => PathStep::Lane(r.must_get_sidewalk(map)),
                PathStepV2::Contraflow(r) => PathStep::ContraflowLane(r.must_get_sidewalk(map)),
                PathStepV2::Movement(mvmnt) => PathStep::Turn(TurnID {
                    src: mvmnt.from.must_get_sidewalk(map),
                    dst: mvmnt.to.must_get_sidewalk(map),
                    parent: mvmnt.parent,
                }),
                PathStepV2::ContraflowMovement(mvmnt) => PathStep::ContraflowTurn(TurnID {
                    src: mvmnt.from.must_get_sidewalk(map),
                    dst: mvmnt.to.must_get_sidewalk(map),
                    parent: mvmnt.parent,
                }),
            });
        }
        Ok(Path::new(map, steps, self.req, Vec::new(), Vec::new()))
    }

    pub fn crosses_road(&self, r: RoadID) -> bool {
        self.steps.iter().any(|step| match step {
            PathStepV2::Along(dr) => dr.road == r,
            _ => false,
        })
    }

    /// Draws the thickened path, matching entire roads. Ignores the path's exact starting and
    /// ending distance. Doesn't handle contraflow yet.
    pub fn trace_v2(&self, map: &Map) -> Result<Polygon> {
        let mut left_pts = Vec::new();
        let mut right_pts = Vec::new();
        for step in &self.steps {
            match step {
                PathStepV2::Along(dr) => {
                    let road = map.get_r(dr.road);
                    let width = road.get_half_width();
                    if dr.dir == Direction::Fwd {
                        left_pts.extend(road.center_pts.shift_left(width)?.into_points());
                        right_pts.extend(road.center_pts.shift_right(width)?.into_points());
                    } else {
                        left_pts
                            .extend(road.center_pts.shift_right(width)?.reversed().into_points());
                        right_pts
                            .extend(road.center_pts.shift_left(width)?.reversed().into_points());
                    }
                }
                PathStepV2::Contraflow(_) => todo!(),
                // Just make a straight line across the intersection. It'd be fancier to try and
                // trace along.
                PathStepV2::Movement(_) | PathStepV2::ContraflowMovement(_) => {}
            }
        }
        right_pts.reverse();
        left_pts.extend(right_pts);
        left_pts.push(left_pts[0]);
        Ok(Ring::deduping_new(left_pts)?.into_polygon())
    }

    /// Returns polygons covering the entire path. Ignores the path's exact starting and ending
    /// distance.
    pub fn trace_all_polygons(&self, map: &Map) -> Vec<Polygon> {
        let mut polygons = Vec::new();
        for step in &self.steps {
            match step {
                PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                    polygons.push(map.get_r(dr.road).get_thick_polygon());
                }
                PathStepV2::Movement(m) | PathStepV2::ContraflowMovement(m) => {
                    polygons.push(map.get_i(m.parent).polygon.clone());
                }
            }
        }
        polygons
    }
}

fn find_uber_turns(
    steps: &[PathStep],
    map: &Map,
    mut uber_turns_v2: Vec<UberTurnV2>,
) -> Vec<UberTurn> {
    // Pathfinding v1 needs to know the uber turns that the path crosses, for the simulation layer.
    // Since we now construct the path in two stages, it's easiest to just reconstruct the uber
    // turns after building the lane-based path.

    let num_uts = uber_turns_v2.len();
    let mut result = Vec::new();
    let mut current_ut = Vec::new();
    for step in steps {
        // Optimization
        if uber_turns_v2.is_empty() {
            break;
        }

        if let PathStep::Turn(t) = step {
            if current_ut.is_empty()
                && uber_turns_v2[0].path[0].from == map.get_l(t.src).get_directed_parent()
            {
                current_ut.push(*t);
            }

            if !current_ut.is_empty() {
                if current_ut.last() != Some(t) {
                    current_ut.push(*t);
                }
                if uber_turns_v2[0].path[0].to == map.get_l(t.dst).get_directed_parent() {
                    result.push(UberTurn {
                        path: current_ut.drain(..).collect(),
                    });
                    uber_turns_v2.remove(0);
                }
            }
        }
    }
    assert!(current_ut.is_empty());
    assert_eq!(num_uts, result.len());
    result
}
