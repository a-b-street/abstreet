//! Structures related to the new road-based pathfinding
//! (https://github.com/a-b-street/abstreet/issues/555) live here. When the transition is done,
//! things here will probably move into pathfind/mod.rs.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use geom::Duration;

use crate::pathfind::uber_turns::UberTurnV2;
use crate::{
    DirectedRoadID, Map, MovementID, Path, PathConstraints, PathRequest, PathStep, TurnID, UberTurn,
};

/// One step along a path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PathStepV2 {
    /// Original direction
    Along(DirectedRoadID),
    /// Opposite direction, sidewalks only
    Contraflow(DirectedRoadID),
    Turn(MovementID),
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
}

impl PathV2 {
    pub(crate) fn new(
        steps: Vec<PathStepV2>,
        req: PathRequest,
        cost: Duration,
        uber_turns: Vec<UberTurnV2>,
    ) -> PathV2 {
        // TODO Port validate_continuity and validate_restrictions?
        PathV2 {
            steps,
            req,
            cost,
            uber_turns,
        }
    }

    /// Vehicle implementations often just calculate the sequence of roads. Turn that into
    /// PathStepV2 here.
    pub(crate) fn from_roads(
        mut roads: Vec<DirectedRoadID>,
        req: PathRequest,
        cost: Duration,
        uber_turns: Vec<UberTurnV2>,
        map: &Map,
    ) -> PathV2 {
        let mut steps = Vec::new();
        for pair in roads.windows(2) {
            steps.push(PathStepV2::Along(pair[0]));
            steps.push(PathStepV2::Turn(MovementID {
                from: pair[0],
                to: pair[1],
                parent: pair[0].dst_i(map),
                crosswalk: false,
            }));
        }
        steps.push(PathStepV2::Along(roads.pop().unwrap()));
        PathV2::new(steps, req, cost, uber_turns)
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
    /// the path might be faster. This time incorporates costs like using sub-optimal lanes or
    /// taking difficult turns.
    pub fn get_cost(&self) -> Duration {
        self.cost
    }

    /// Transform a sequence of roads representing a path into the current lane-based path, by
    /// picking particular lanes and turns to use.
    pub fn to_v1(self, map: &Map) -> Result<Path> {
        if self.req.constraints == PathConstraints::Pedestrian {
            return self.to_v1_walking(map);
        }

        // This is a somewhat brute-force method: run Dijkstra's algorithm on a graph of lanes and
        // turns, but only build the graph along the path of roads we've already found. This handles
        // arbitrary lookahead needed, and forces use of the original start/end lanes requested.
        let mut graph = petgraph::graphmap::DiGraphMap::new();
        for step in &self.steps {
            if let PathStepV2::Turn(mvmnt) = step {
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

        match petgraph::algo::astar(
            &graph,
            self.req.start.lane(),
            |l| l == self.req.end.lane(),
            |(_, _, t)| {
                // Normally opportunistic lane-changing adjusts the path live, but that doesn't work
                // near uber-turns. So still use some of the penalties here.
                let (lt, lc, slow_lane) = map.get_t(*t).penalty(map);
                let mut extra_penalty = lt + lc;
                if self.req.constraints == PathConstraints::Bike {
                    extra_penalty = slow_lane;
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
                for pair in path.windows(2) {
                    steps.push(PathStep::Lane(pair[0]));
                    // We don't need to look for this turn in the map; we know it exists.
                    steps.push(PathStep::Turn(TurnID {
                        parent: map.get_l(pair[0]).dst_i,
                        src: pair[0],
                        dst: pair[1],
                    }));
                }
                steps.push(PathStep::Lane(self.req.end.lane()));
                assert_eq!(steps[0], PathStep::Lane(self.req.start.lane()));
                let uber_turns = find_uber_turns(&steps, map, self.uber_turns);
                Ok(Path::new(map, steps, self.req, uber_turns))
            }
            None => bail!(
                "Can't transform a road-based path to a lane-based path for {}",
                self.req
            ),
        }
    }

    fn to_v1_walking(self, map: &Map) -> Result<Path> {
        let mut steps = Vec::new();
        for step in self.steps {
            steps.push(match step {
                PathStepV2::Along(r) => PathStep::Lane(r.must_get_sidewalk(map)),
                PathStepV2::Contraflow(r) => PathStep::ContraflowLane(r.must_get_sidewalk(map)),
                PathStepV2::Turn(mvmnt) => PathStep::Turn(TurnID {
                    src: mvmnt.from.must_get_sidewalk(map),
                    dst: mvmnt.to.must_get_sidewalk(map),
                    parent: mvmnt.parent,
                }),
            });
        }
        Ok(Path::new(map, steps, self.req, Vec::new()))
    }
}

fn find_uber_turns(
    steps: &Vec<PathStep>,
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
            if current_ut.is_empty() {
                if uber_turns_v2[0].path[0].from == map.get_l(t.src).get_directed_parent() {
                    current_ut.push(*t);
                }
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
