//! Structures related to the new road-based pathfinding
//! (https://github.com/a-b-street/abstreet/issues/555) live here. When the transition is done,
//! things here will probably move into pathfind/mod.rs.

use anyhow::Result;

use crate::pathfind::uber_turns::UberTurnV2;
use crate::{DirectedRoadID, Map, Path, PathConstraints, PathRequest, PathStep, TurnID, UberTurn};

/// Transform a sequence of roads representing a path into the current lane-based path, by picking
/// particular lanes and turns to use.
pub fn path_v2_to_v1(
    req: PathRequest,
    road_steps: Vec<DirectedRoadID>,
    uber_turns_v2: Vec<UberTurnV2>,
    map: &Map,
) -> Result<Path> {
    // This is a somewhat brute-force method: run Dijkstra's algorithm on a graph of lanes and
    // turns, but only build the graph along the path of roads we've already found. This handles
    // arbitrary lookahead needed, and forces use of the original start/end lanes requested.
    //
    // Eventually we'll directly return road-based paths. Most callers will actually just use that
    // directly, and mainly the simulation will need to expand to specific lanes, but it'll do so
    // dynamically/lazily to account for current traffic conditions.
    let mut graph = petgraph::graphmap::DiGraphMap::new();
    for pair in road_steps.windows(2) {
        for src in pair[0].lanes(req.constraints, map) {
            for dst in pair[1].lanes(req.constraints, map) {
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

    match petgraph::algo::astar(
        &graph,
        req.start.lane(),
        |l| l == req.end.lane(),
        |(_, _, t)| {
            // Normally opportunistic lane-changing adjusts the path live, but that doesn't work
            // near uber-turns. So still use some of the penalties here.
            let (lt, lc, slow_lane) = map.get_t(*t).penalty(map);
            let mut extra_penalty = lt + lc;
            if req.constraints == PathConstraints::Bike {
                extra_penalty = slow_lane;
            }
            // Always treat every lane/turn as at least cost 1; otherwise A* can't understand that
            // a final path with 10 steps costs more than one with 5. The road-based pathfinding
            // has already chosen the overall route; when we're picking individual lanes, the
            // length of each lane along one road is going to be about the same.
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
            steps.push(PathStep::Lane(req.end.lane()));
            assert_eq!(steps[0], PathStep::Lane(req.start.lane()));
            let uber_turns = find_uber_turns(&steps, map, uber_turns_v2);
            Ok(Path::new(map, steps, req, uber_turns))
        }
        None => bail!(
            "path_v2_to_v1 found road-based path, but not a lane-based path matching it for {}",
            req
        ),
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
