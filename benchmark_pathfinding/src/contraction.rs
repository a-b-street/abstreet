use abstutil::Timer;
use map_model::{LaneID, Map, PathRequest, TurnID};
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64;

type Weight = f64;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
enum Node {
    Start(LaneID),
    End(LaneID),
}

#[derive(Serialize, Deserialize)]
enum Edge {
    CrossLane(LaneID, Weight),
    ContraflowCrossLane(LaneID, Weight),
    // Also store (from, to)
    CrossTurn(TurnID, Node, Node, Weight),
    Shortcut(Weight),
}

impl Edge {
    fn get_weight(&self) -> f64 {
        match self {
            Edge::CrossLane(_, weight) => *weight,
            Edge::ContraflowCrossLane(_, weight) => *weight,
            Edge::CrossTurn(_, _, _, weight) => *weight,
            Edge::Shortcut(weight) => *weight,
        }
    }
}

pub fn build_ch(path: String, map: &Map, timer: &mut Timer) {
    // TODO Not sure petgraph is helping me at all. No notion of ordered edge weight, no looking up
    // nodes.
    let mut g: StableGraph<Node, Edge> = StableGraph::new();
    let mut nodes: HashMap<Node, NodeIndex<u32>> = HashMap::new();

    for l in map.all_lanes() {
        let start = g.add_node(Node::Start(l.id));
        let end = g.add_node(Node::End(l.id));
        nodes.insert(Node::Start(l.id), start);
        nodes.insert(Node::End(l.id), end);

        g.add_edge(start, end, Edge::CrossLane(l.id, l.length().inner_meters()));
        if l.is_sidewalk() {
            g.add_edge(
                end,
                start,
                Edge::ContraflowCrossLane(l.id, l.length().inner_meters()),
            );
        }
    }
    for t in map.all_turns().values() {
        let src = if map.get_l(t.id.src).dst_i == t.id.parent {
            Node::End(t.id.src)
        } else {
            Node::Start(t.id.src)
        };
        let dst = if map.get_l(t.id.dst).src_i == t.id.parent {
            Node::Start(t.id.dst)
        } else {
            Node::End(t.id.dst)
        };
        g.add_edge(
            nodes[&src],
            nodes[&dst],
            Edge::CrossTurn(t.id, src, dst, t.geom.length().inner_meters()),
        );
    }

    println!(
        "{} nodes, {} edges, is directed {}",
        g.node_count(),
        g.edge_count(),
        g.is_directed()
    );

    // Nodes are numbered increasing as we contract them.
    let mut node_order: HashMap<NodeIndex<u32>, usize> = HashMap::new();

    // Contract nodes in a random order (because of hash iteration)
    timer.start_iter("contracting nodes", nodes.len());
    for (order, (_, id)) in nodes.iter().enumerate() {
        timer.next();
        node_order.insert(*id, order);

        let predecessors: Vec<NodeIndex<u32>> =
            g.neighbors_directed(*id, Direction::Incoming).collect();
        let successors: Vec<NodeIndex<u32>> =
            g.neighbors_directed(*id, Direction::Outgoing).collect();

        for pred in predecessors {
            if node_order.contains_key(&pred) {
                continue;
            }
            for succ in &successors {
                if node_order.contains_key(succ) {
                    continue;
                }
                // Find the shortest path from pred to succ WITHOUT using anything already
                // contracted (in node_order). We know the path must exist -- pred->node->succ, at
                // the very least.
                // TODO Do my own pathfinding, so I can properly skip nodes instead of doing the
                // f64::MAX hack.
                let (total_cost, path) = petgraph::algo::astar(
                    &g,
                    pred,
                    |finish| finish == *succ,
                    |e| {
                        if node_order.contains_key(&e.source())
                            || node_order.contains_key(&e.target())
                        {
                            f64::MAX
                        } else {
                            e.weight().get_weight()
                        }
                    },
                    |_| 0.0,
                )
                .unwrap();

                // If the path winds up being [pred, node, succ], then add a shortcut edge with the
                // sum weight.
                if path.len() == 3 && path[1] == *id {
                    g.add_edge(pred, *succ, Edge::Shortcut(total_cost));
                }
            }
        }
    }

    println!(
        "\n{} nodes, {} edges, is directed {}",
        g.node_count(),
        g.edge_count(),
        g.is_directed()
    );

    let graph = CHGraph {
        graph: g,
        nodes,
        node_order,
    };
    abstutil::write_binary(&path, &graph).unwrap();
}

#[derive(Serialize, Deserialize)]
pub struct CHGraph {
    graph: StableGraph<Node, Edge>,
    nodes: HashMap<Node, NodeIndex<u32>>,
    node_order: HashMap<NodeIndex<u32>, usize>,
}

impl CHGraph {
    pub fn pathfind(&self, req: &PathRequest) {
        let start_node = self.nodes[&Node::Start(req.start.lane())];
        let end_node = self.nodes[&Node::Start(req.end.lane())];
        if let Some((_total_cost, _path)) = petgraph::algo::astar(
            &self.graph,
            start_node,
            |finish| finish == end_node,
            |e| e.weight().get_weight(),
            |_| 0.0,
        ) {
            //println!("path costs {}, is {:?}", total_cost, path);
        } else {
            //println!("Couldn't find path from {} to {}", start, end);
        }
    }
}
