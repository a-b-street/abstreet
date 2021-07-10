use std::cell::RefCell;

use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};
use thread_local::ThreadLocal;

/// This operates on raw IDs and costs; no type safety. The thing containing this transforms
/// to/from higher-level types.
#[derive(Serialize, Deserialize)]
pub enum PathfindEngine {
    Dijkstra {
        // TODO It's not serializable?!
        #[serde(skip_serializing, skip_deserializing)]
        graph: DiGraphMap<usize, usize>,
    },
    CH {
        #[serde(serialize_with = "serialize_32", deserialize_with = "deserialize_32")]
        graph: FastGraph,
        #[serde(skip_serializing, skip_deserializing)]
        path_calc: ThreadLocal<RefCell<PathCalculator>>,
    },
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
            PathfindEngine::Dijkstra { ref graph } => {
                // TODO Handle multiple sources/targets by brute-force
                petgraph::algo::astar(
                    graph,
                    starts[0].0,
                    |node| node == ends[0].0,
                    |(_, _, cost)| *cost,
                    |_| 0,
                )
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
        todo!()
    }
}

#[derive(Clone, Copy)]
pub enum CreateEngine {
    Dijkstra,
    CH,
}

impl CreateEngine {
    pub fn create(self, input_graph: InputGraph) -> PathfindEngine {
        match self {
            CreateEngine::Dijkstra => {
                let mut graph = DiGraphMap::new();
                for edge in input_graph.get_edges() {
                    graph.add_edge(edge.from, edge.to, edge.weight);
                }
                PathfindEngine::Dijkstra { graph }
            }
            CreateEngine::CH => {
                info!(
                    "Contraction hierarchy input graph has {} nodes",
                    abstutil::prettyprint_usize(input_graph.get_num_nodes())
                );

                PathfindEngine::CH {
                    graph: fast_paths::prepare(&input_graph),
                    path_calc: ThreadLocal::new(),
                }
            }
        }
    }
}

/*let graph = if let Some(seed) = seed {
           let node_ordering = seed.graph.get_node_ordering();
           fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap()
       } else {
           fast_paths::prepare(&input_graph)
       };

*/
