use crate::pathfind::{
    cost, one_step_walking_path, walking_cost, walking_path_to_steps, WalkingNode,
};
use crate::{
    IntersectionID, LaneID, Map, Path, PathConstraints, PathRequest, PathStep, RoadID, TurnID,
};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ZoneID(pub usize);

impl fmt::Display for ZoneID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Zone #{}", self.0)
    }
}

// A contiguous set of roads with access restrictions
#[derive(Serialize, Deserialize, Debug)]
pub struct Zone {
    pub id: ZoneID,
    pub members: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
}

impl Zone {
    // Run slower Dijkstra's within the interior of a private zone. Don't go outside the borders.
    pub(crate) fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        // TODO Not happy this works so differently
        if req.constraints == PathConstraints::Pedestrian {
            return self.pathfind_walking(req, map);
        }

        let mut graph: DiGraphMap<LaneID, TurnID> = DiGraphMap::new();
        for r in &self.members {
            for l in map.get_r(*r).all_lanes() {
                if req.constraints.can_use(map.get_l(l), map) {
                    for turn in map.get_turns_for(l, req.constraints) {
                        if !self.borders.contains(&turn.id.parent) {
                            graph.add_edge(turn.id.src, turn.id.dst, turn.id);
                        }
                    }
                }
            }
        }

        let (_, path) = petgraph::algo::astar(
            &graph,
            req.start.lane(),
            |l| l == req.end.lane(),
            |(_, _, turn)| cost(map.get_l(turn.src), map.get_t(*turn), req.constraints, map),
            |_| 0,
        )?;
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
        Some(Path::new(map, steps, req.end.dist_along()))
    }

    fn pathfind_walking(&self, req: PathRequest, map: &Map) -> Option<Path> {
        if req.start.lane() == req.end.lane() {
            return Some(one_step_walking_path(&req, map));
        }

        let mut graph: DiGraphMap<WalkingNode, usize> = DiGraphMap::new();
        for r in &self.members {
            for l in map.get_r(*r).all_lanes() {
                let l = map.get_l(l);
                if l.is_sidewalk() {
                    let cost = walking_cost(l.length());
                    let n1 = WalkingNode::SidewalkEndpoint(l.id, true);
                    let n2 = WalkingNode::SidewalkEndpoint(l.id, false);
                    graph.add_edge(n1, n2, cost);
                    graph.add_edge(n2, n1, cost);

                    for turn in map.get_turns_for(l.id, PathConstraints::Pedestrian) {
                        if self.members.contains(&map.get_l(turn.id.dst).parent) {
                            graph.add_edge(
                                WalkingNode::SidewalkEndpoint(l.id, l.dst_i == turn.id.parent),
                                WalkingNode::SidewalkEndpoint(
                                    turn.id.dst,
                                    map.get_l(turn.id.dst).dst_i == turn.id.parent,
                                ),
                                walking_cost(turn.geom.length()),
                            );
                        }
                    }
                }
            }
        }

        let closest_start = WalkingNode::closest(req.start, map);
        let closest_end = WalkingNode::closest(req.end, map);
        let (_, path) = petgraph::algo::astar(
            &graph,
            closest_start,
            |end| end == closest_end,
            |(_, _, cost)| *cost,
            |_| 0,
        )?;
        let steps = walking_path_to_steps(path, map);
        assert_eq!(steps[0].as_lane(), req.start.lane());
        assert_eq!(steps.last().unwrap().as_lane(), req.end.lane());
        Some(Path::new(map, steps, req.end.dist_along()))
    }
}
