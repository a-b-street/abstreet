use fast_paths::{NodeId, ShortestPath};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;

// TODO Upstream this in fast_paths when this is more solid.
#[derive(Serialize)]
pub struct NodeMap<T: Copy + Ord + Debug + Serialize> {
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
}

// TODO Still can't figure out how to derive Deserialize on NodeMap directly.
pub fn deserialize_nodemap<
    'de,
    D: Deserializer<'de>,
    T: Deserialize<'de> + Copy + Ord + Debug + Serialize,
>(
    d: D,
) -> Result<NodeMap<T>, D::Error> {
    // TODO I'm offline and can't look up hw to use Deserializer twice in sequence. Since the two
    // fields are redundant, just serialize one of them.
    let id_to_node = <Vec<T>>::deserialize(d)?;
    let mut node_to_id = BTreeMap::new();
    for (id, node) in id_to_node.iter().enumerate() {
        node_to_id.insert(*node, id);
    }

    Ok(NodeMap {
        node_to_id,
        id_to_node,
    })
}
