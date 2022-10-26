use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

/// Use with `BinaryHeap`. Since it's a max-heap, reverse the comparison to get the smallest cost
/// first.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PriorityQueueItem<K, V> {
    pub cost: K,
    pub value: V,
}

impl<K: Ord, V: Ord> PartialOrd for PriorityQueueItem<K, V> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ord, V: Ord> Ord for PriorityQueueItem<K, V> {
    fn cmp(&self, other: &Self) -> Ordering {
        let ord = other.cost.cmp(&self.cost);
        if ord != Ordering::Equal {
            return ord;
        }
        // The tie-breaker is arbitrary, based on the value
        self.value.cmp(&other.value)
    }
}
