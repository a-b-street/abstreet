use std;
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct MultiMap<K, V> {
    map: HashMap<K, HashSet<V>>,
    empty: HashSet<V>,
}

impl<K, V> MultiMap<K, V>
where
    K: std::cmp::Eq + std::hash::Hash,
    V: std::cmp::Eq + std::hash::Hash,
{
    pub fn new() -> MultiMap<K, V> {
        MultiMap {
            map: HashMap::new(),
            empty: HashSet::new(),
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.map
            .entry(key)
            .or_insert_with(HashSet::new)
            .insert(value);
    }

    pub fn remove(&mut self, key: K, value: V) {
        if !self.map.contains_key(&key) {
            return;
        }
        self.map.get_mut(&key).unwrap().remove(&value);
        if self.map[&key].is_empty() {
            self.map.remove(&key);
        }
    }

    pub fn get(&self, key: K) -> &HashSet<V> {
        self.map.get(&key).unwrap_or(&self.empty)
    }
}

pub fn wraparound_get<T>(vec: &Vec<T>, idx: isize) -> &T {
    let len = vec.len() as isize;
    let idx = idx % len;
    let idx = if idx >= 0 { idx } else { idx + len };
    &vec[idx as usize]
}

pub fn retain_btreemap<K: Ord + Clone, V, F: Fn(&K, &V) -> bool>(
    map: &mut BTreeMap<K, V>,
    keep: F,
) {
    let mut remove_keys: Vec<K> = Vec::new();
    for (k, v) in map.iter() {
        if !keep(k, v) {
            remove_keys.push(k.clone());
        }
    }
    for k in remove_keys {
        map.remove(&k);
    }
}
