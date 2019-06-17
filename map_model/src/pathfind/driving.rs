use crate::{DirectedRoadID, LaneID, LaneType, Map, Path, PathRequest, PathStep, Turn, TurnID};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Distance;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

// TODO Make the graph smaller by considering RoadID, or even (directed?) bundles of roads based on
// OSM way.
#[derive(Serialize, Deserialize)]
pub struct VehiclePathfinder {
    graph: StableGraph<DirectedRoadID, Distance>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    nodes: BTreeMap<DirectedRoadID, NodeIndex<u32>>,
    lane_types: Vec<LaneType>,
}

pub enum Outcome {
    Success(Path),
    Failure,
    RetrySlow,
}

impl VehiclePathfinder {
    pub fn new(map: &Map, lane_types: Vec<LaneType>) -> VehiclePathfinder {
        let mut g = VehiclePathfinder {
            graph: StableGraph::new(),
            nodes: BTreeMap::new(),
            lane_types,
        };

        for r in map.all_roads() {
            // Could omit if there aren't any matching lane types, but since those can be edited,
            // it's actually a bit simpler to just have all nodes.
            if !r.children_forwards.is_empty() {
                let id = r.id.forwards();
                g.nodes.insert(id, g.graph.add_node(id));
            }
            if !r.children_backwards.is_empty() {
                let id = r.id.backwards();
                g.nodes.insert(id, g.graph.add_node(id));
            }
        }

        for t in map.all_turns().values() {
            g.add_turn(t, map);
        }

        /*println!(
            "{} nodes, {} edges",
            g.graph.node_count(),
            g.graph.edge_count()
        );*/

        g
    }

    fn add_turn(&mut self, t: &Turn, map: &Map) {
        if !map.is_turn_allowed(t.id) {
            return;
        }
        let src_l = map.get_l(t.id.src);
        let dst_l = map.get_l(t.id.dst);
        if self.lane_types.contains(&src_l.lane_type) && self.lane_types.contains(&dst_l.lane_type)
        {
            let src = self.get_node(t.id.src, map);
            let dst = self.get_node(t.id.dst, map);
            // First length arbitrarily wins.
            if self.graph.find_edge(src, dst).is_none() {
                self.graph
                    .add_edge(src, dst, src_l.length() + t.geom.length());
            }
        }
    }

    fn get_node(&self, lane: LaneID, map: &Map) -> NodeIndex<u32> {
        self.nodes[&map.get_l(lane).get_directed_parent(map)]
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Outcome {
        assert!(!map.get_l(req.start.lane()).is_sidewalk());

        let start_node = self.get_node(req.start.lane(), map);
        let end_node = self.get_node(req.end.lane(), map);
        let end_pt = map.get_l(req.end.lane()).first_pt();

        let raw_nodes = match petgraph::algo::astar(
            &self.graph,
            start_node,
            |n| n == end_node,
            |e| *e.weight(),
            |n| {
                let dr = self.graph[n];
                let r = map.get_r(dr.id);
                if dr.forwards {
                    end_pt.dist_to(r.center_pts.last_pt())
                } else {
                    end_pt.dist_to(r.center_pts.first_pt())
                }
            },
        ) {
            Some((_, nodes)) => nodes,
            None => {
                return Outcome::Failure;
            }
        };

        // TODO windows(2) would be fine for peeking, except it drops the last element for odd
        // cardinality
        let mut nodes = VecDeque::from(raw_nodes);

        let mut steps: Vec<PathStep> = Vec::new();
        while !nodes.is_empty() {
            let n = nodes.pop_front().unwrap();
            let dr = self.graph[n];
            if steps.is_empty() {
                steps.push(PathStep::Lane(req.start.lane()));
            } else {
                let from_lane = match steps.last() {
                    Some(PathStep::Lane(l)) => *l,
                    _ => unreachable!(),
                };
                if let Some(turn) = map.get_turns_from_lane(from_lane).into_iter().find(|t| {
                    // Special case the last step
                    if nodes.is_empty() {
                        t.id.dst == req.end.lane()
                    } else {
                        let l = map.get_l(t.id.dst);
                        if l.get_directed_parent(map) == dr {
                            // TODO different case when nodes.len() == 1.
                            map.get_turns_from_lane(l.id).into_iter().any(|t2| {
                                map.get_l(t2.id.dst).get_directed_parent(map)
                                    == self.graph[nodes[0]]
                            })
                        } else {
                            false
                        }
                    }
                }) {
                    steps.push(PathStep::Turn(turn.id));
                    steps.push(PathStep::Lane(turn.id.dst));
                } else {
                    if steps.len() == 1 {
                        // Started in the wrong lane
                        return Outcome::RetrySlow;
                    } else {
                        // Need more lookahead to stitch together the right path
                        return Outcome::RetrySlow;
                    }
                }
            }
        }
        Outcome::Success(Path::new(map, steps, req.end.dist_along()))
    }

    pub fn apply_edits(
        &mut self,
        delete_turns: &BTreeSet<TurnID>,
        add_turns: &BTreeSet<TurnID>,
        map: &Map,
    ) {
        // Most turns will be in both lists. That's fine -- we want to re-add the same turn and
        // check if the lane type is different.
        for t in delete_turns {
            if let Some(e) = self
                .graph
                .find_edge(self.get_node(t.src, map), self.get_node(t.dst, map))
            {
                self.graph.remove_edge(e);
            }
        }

        for t in add_turns {
            self.add_turn(map.get_t(*t), map);
        }
    }
}
