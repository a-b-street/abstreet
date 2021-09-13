use std::cell::RefCell;
use std::collections::HashMap;

use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use thread_local::ThreadLocal;

/// This operates on raw IDs and costs; no type safety. The thing containing this transforms
/// to/from higher-level types.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize)]
pub enum PathfindEngine {
    Empty,
    Dijkstra {
        graph: DiGraph<usize, usize>,
    },
    CH {
        #[serde(serialize_with = "serialize_32", deserialize_with = "deserialize_32")]
        graph: FastGraph,
        #[serde(skip_serializing, skip_deserializing)]
        path_calc: ThreadLocal<RefCell<PathCalculator>>,
    },
}

// Implemented manually to deal with the ThreadLocal
impl Clone for PathfindEngine {
    fn clone(&self) -> Self {
        match self {
            PathfindEngine::Empty => PathfindEngine::Empty,
            PathfindEngine::Dijkstra { ref graph } => PathfindEngine::Dijkstra {
                graph: graph.clone(),
            },
            PathfindEngine::CH { ref graph, .. } => PathfindEngine::CH {
                graph: graph.clone(),
                path_calc: ThreadLocal::new(),
            },
        }
    }
}

impl PathfindEngine {
    /// Returns (path cost, node IDs in path)
    pub fn calculate_path(&self, start: usize, end: usize) -> Option<(usize, Vec<usize>)> {
        self.calculate_path_multiple_sources_and_targets(vec![(start, 0)], vec![(end, 0)])
    }

    /// Returns (path cost, node IDs in path). Input is pairs of (node ID, extra weight)
    pub fn calculate_path_multiple_sources_and_targets(
        &self,
        starts: Vec<(usize, usize)>,
        ends: Vec<(usize, usize)>,
    ) -> Option<(usize, Vec<usize>)> {
        match self {
            PathfindEngine::Empty => unreachable!(),
            PathfindEngine::Dijkstra { ref graph } => {
                // If there are multiple starts and ends, calculate each individual path and take
                // the lowest cost.
                let mut best_pair: Option<(usize, Vec<NodeIndex>)> = None;
                for (start_node, weight1) in starts {
                    let start_node = NodeIndex::new(start_node);
                    for (end_node, weight2) in &ends {
                        let end_node = NodeIndex::new(*end_node);
                        if let Some((raw_weight, raw_nodes)) = petgraph::algo::astar(
                            graph,
                            start_node,
                            |node| node == end_node,
                            |edge| *edge.weight(),
                            |_| 0,
                        ) {
                            let total_weight = raw_weight + weight1 + weight2;
                            if best_pair
                                .as_ref()
                                .map(|pair| total_weight < pair.0)
                                .unwrap_or(true)
                            {
                                best_pair = Some((total_weight, raw_nodes));
                            }
                        }
                    }
                }
                let (raw_weight, raw_nodes) = best_pair?;
                Some((
                    raw_weight,
                    raw_nodes.into_iter().map(|n| n.index()).collect(),
                ))
            }
            PathfindEngine::CH {
                ref graph,
                ref path_calc,
            } => {
                let mut calc = path_calc
                    .get_or(|| RefCell::new(fast_paths::create_calculator(graph)))
                    .borrow_mut();
                let path = calc.calc_path_multiple_sources_and_targets(graph, starts, ends)?;
                // TODO Add an into_nodes to avoid this clone
                Some((path.get_weight(), path.get_nodes().to_vec()))
            }
        }
    }

    pub fn reuse_ordering(&self) -> CreateEngine {
        match self {
            PathfindEngine::Empty => unreachable!(),
            // Just don't reuse the ordering
            PathfindEngine::Dijkstra { .. } => CreateEngine::Dijkstra,
            PathfindEngine::CH { ref graph, .. } => CreateEngine::CHSeedingNodeOrdering(graph),
        }
    }

    pub fn is_dijkstra(&self) -> bool {
        matches!(self, PathfindEngine::Dijkstra { .. })
    }

    pub fn all_costs_from(&self, start: usize) -> HashMap<usize, usize> {
        match self {
            PathfindEngine::Empty => unreachable!(),
            PathfindEngine::Dijkstra { ref graph } => {
                petgraph::algo::dijkstra(graph, NodeIndex::new(start), None, |edge| *edge.weight())
                    .into_iter()
                    .map(|(k, v)| (k.index(), v))
                    .collect()
            }
            PathfindEngine::CH { .. } => unreachable!(),
        }
    }
}

pub enum CreateEngine<'a> {
    Dijkstra,
    CH,
    CHSeedingNodeOrdering(&'a FastGraph),
}

impl<'a> CreateEngine<'a> {
    pub fn create(&self, input_graph: InputGraph) -> PathfindEngine {
        match self {
            CreateEngine::Dijkstra => {
                let mut graph = DiGraph::new();
                let dummy_weight = 42;
                for node in 0..input_graph.get_num_nodes() {
                    assert_eq!(graph.add_node(dummy_weight).index(), node);
                }
                for edge in input_graph.get_edges() {
                    graph.add_edge(
                        NodeIndex::new(edge.from),
                        NodeIndex::new(edge.to),
                        edge.weight,
                    );
                }
                PathfindEngine::Dijkstra { graph }
            }
            CreateEngine::CH => {
                info!(
                    "Contraction hierarchy input graph has {} nodes",
                    abstutil::prettyprint_usize(input_graph.get_num_nodes())
                );

                PathfindEngine::CH {
                    graph: fast_paths::prepare_with_params(
                        &input_graph,
                        // see discussion about fast_paths parameters here: https://github.com/easbar/fast_paths/pull/37
                        &fast_paths::Params::new(0.01, 100, 10, 100),
                    ),
                    path_calc: ThreadLocal::new(),
                }
            }
            CreateEngine::CHSeedingNodeOrdering(prev_graph) => {
                let node_ordering = prev_graph.get_node_ordering();
                let graph = fast_paths::prepare_with_order_with_params(
                    &input_graph,
                    &node_ordering,
                    &fast_paths::ParamsWithOrder::new(100),
                )
                .unwrap();
                PathfindEngine::CH {
                    graph,
                    path_calc: ThreadLocal::new(),
                }
            }
        }
    }
}
