//! Some helpers for working with fast_paths.

use std::collections::BTreeMap;
use std::fmt::Debug;

use fast_paths::{InputGraph, NodeId, ShortestPath};
use serde::{Deserialize, Deserializer, Serialize};

/// A bidirectional mapping between fast_paths NodeId and some custom ID type.
// TODO Upstream this in fast_paths when this is more solid.
#[derive(Serialize)]
pub struct NodeMap<T: Copy + Ord + Debug + Serialize> {
    // These two fields are redundant and large, so don't serialize the bigger one, to cut down
    // file size.
    #[serde(skip_serializing)]
    node_to_id: BTreeMap<T, NodeId>,
    id_to_node: Vec<T>,
}

impl<T: Copy + Ord + Debug + Serialize> NodeMap<T> {
    pub fn new() -> NodeMap<T> {
        NodeMap {
            node_to_id: BTreeMap::new(),
            id_to_node: Vec::new(),
        }
    }

    pub fn get_or_insert(&mut self, node: T) -> NodeId {
        if let Some(id) = self.node_to_id.get(&node) {
            return *id;
        }
        let id = self.id_to_node.len();
        self.node_to_id.insert(node, id);
        self.id_to_node.push(node);
        id
    }

    pub fn get(&self, node: T) -> NodeId {
        if let Some(id) = self.node_to_id.get(&node) {
            *id
        } else {
            panic!("{:?} not in NodeMap", node);
        }
    }

    pub fn translate(&self, path: &ShortestPath) -> Vec<T> {
        path.get_nodes()
            .iter()
            .map(|id| self.id_to_node[*id])
            .collect()
    }

    /// Call this after filling out the input graph, right before preparation.
    pub fn guarantee_node_ordering(&self, input_graph: &mut InputGraph) {
        // The fast_paths implementation will trim out the last nodes in the input graph if there
        // are no edges involving them:
        // https://github.com/easbar/fast_paths/blob/fdb65f25c5485c9c74c1b3cbe66d829eea81b14b/src/input_graph.rs#L151
        //
        // We sometimes add nodes that aren't used yet, so that we can reuse the same node ordering
        // later. Detect if the last node isn't used.
        let last_node = self.id_to_node.len() - 1;
        input_graph.freeze();
        for edge in input_graph.get_edges() {
            if edge.from == last_node || edge.to == last_node {
                // The last node is used, so we're fine
                input_graph.thaw();
                return;
            }
        }
        input_graph.thaw();

        // Add a dummy edge from this unused node to any arbitrary node (namely the first), to
        // prevent it from getting trimmed out. Since no path will start or end from this unused
        // node, this won't affect resulting paths.
        let first_node = 0;
        input_graph.add_edge(last_node, first_node, 1);
    }
}

// A serialized NodeMap has this form in JSON. Use this to deserialize.
#[derive(Deserialize)]
struct InnerNodeMap<T: Copy + Ord + Debug> {
    id_to_node: Vec<T>,
}

pub fn deserialize_nodemap<
    'de,
    D: Deserializer<'de>,
    T: Deserialize<'de> + Copy + Ord + Debug + Serialize,
>(
    d: D,
) -> Result<NodeMap<T>, D::Error> {
    let inner = <InnerNodeMap<T>>::deserialize(d)?;
    let id_to_node = inner.id_to_node;
    let mut node_to_id = BTreeMap::new();
    for (id, node) in id_to_node.iter().enumerate() {
        node_to_id.insert(*node, id);
    }

    Ok(NodeMap {
        node_to_id,
        id_to_node,
    })
}
