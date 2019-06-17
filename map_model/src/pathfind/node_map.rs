use bimap::btree::BiBTreeMap;
use fast_paths::{NodeId, ShortestPath};
use serde::{Deserialize, Deserializer, Serialize};

// TODO Upstream this in fast_paths when this is more solid.
#[derive(Serialize)]
pub struct NodeMap<T: Copy + Ord + Serialize> {
    // BiBTreeMap will serialize deterministically, so use it instead of the BiHashMap.
    // TODO Since NodeId is just a usize, maybe have Vec<T> and BTreeMap<T, NodeId> instead of a
    // dependency on bimap.
    nodes: BiBTreeMap<T, NodeId>,
}

impl<T: Copy + Ord + Serialize> NodeMap<T> {
    pub fn new() -> NodeMap<T> {
        NodeMap {
            nodes: BiBTreeMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, node: T) -> NodeId {
        let _ = self.nodes.insert_no_overwrite(node, self.nodes.len());
        *self.nodes.get_by_left(&node).unwrap()
    }

    pub fn get(&self, node: T) -> NodeId {
        *self.nodes.get_by_left(&node).unwrap()
    }

    pub fn translate(&self, path: &ShortestPath) -> Vec<T> {
        path.get_nodes()
            .iter()
            .map(|id| *self.nodes.get_by_right(id).unwrap())
            .collect()
    }
}

// TODO Still can't figure out how to derive Deserialize on NodeMap directly.
pub fn deserialize_nodemap<
    'de,
    D: Deserializer<'de>,
    T: Deserialize<'de> + Copy + Ord + Serialize,
>(
    d: D,
) -> Result<NodeMap<T>, D::Error> {
    let nodes = <BiBTreeMap<T, NodeId>>::deserialize(d)?;
    Ok(NodeMap { nodes })
}
