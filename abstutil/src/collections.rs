use std::cmp::Ord;
use std::collections::{BTreeMap, BTreeSet};

// TODO Ideally derive Serialize and Deserialize, but I can't seem to express the lifetimes
// correctly.
#[derive(PartialEq)]
pub struct MultiMap<K, V>
where
    K: Ord + PartialEq,
    V: Ord + PartialEq,
{
    map: BTreeMap<K, BTreeSet<V>>,
    empty: BTreeSet<V>,
}

impl<K, V> MultiMap<K, V>
where
    K: Ord + PartialEq,
    V: Ord + PartialEq,
{
    pub fn new() -> MultiMap<K, V> {
        MultiMap {
            map: BTreeMap::new(),
            empty: BTreeSet::new(),
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.map
            .entry(key)
            .or_insert_with(BTreeSet::new)
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

    pub fn get(&self, key: K) -> &BTreeSet<V> {
        self.map.get(&key).unwrap_or(&self.empty)
    }

    pub(crate) fn raw_map(&self) -> &BTreeMap<K, BTreeSet<V>> {
        &self.map
    }

    pub fn consume(self) -> BTreeMap<K, BTreeSet<V>> {
        self.map
    }
}

pub struct Counter<T: Ord + PartialEq> {
    map: BTreeMap<T, usize>,
}

impl<T: Ord + PartialEq> Default for Counter<T> {
    fn default() -> Counter<T> {
        Counter::new()
    }
}

impl<T: Ord + PartialEq> Counter<T> {
    pub fn new() -> Counter<T> {
        Counter {
            map: BTreeMap::new(),
        }
    }

    pub fn inc(&mut self, val: T) -> usize {
        let entry = self.map.entry(val).or_insert(0);
        *entry += 1;
        *entry
    }

    pub fn get(&self, val: T) -> usize {
        self.map.get(&val).cloned().unwrap_or(0)
    }

    pub fn sorted_asc(&self) -> Vec<&T> {
        let mut list = self.map.iter().collect::<Vec<_>>();
        list.sort_by_key(|(_, cnt)| *cnt);
        list.into_iter().map(|(t, _)| t).collect()
    }

    pub fn consume(self) -> BTreeMap<T, usize> {
        self.map
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

pub fn retain_btreeset<K: Ord + Clone, F: FnMut(&K) -> bool>(set: &mut BTreeSet<K>, mut keep: F) {
    let mut remove: Vec<K> = Vec::new();
    for k in set.iter() {
        if !keep(k) {
            remove.push(k.clone());
        }
    }
    for k in remove {
        set.remove(&k);
    }
}

pub fn contains_duplicates<T: Ord>(vec: &Vec<T>) -> bool {
    let mut set = BTreeSet::new();
    for item in vec {
        if set.contains(item) {
            return true;
        }
        set.insert(item);
    }
    false
}
