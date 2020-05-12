use std::cmp::Ord;
use std::collections::{BTreeMap, BTreeSet};

// TODO Ideally derive Serialize and Deserialize, but I can't seem to express the lifetimes
// correctly.
#[derive(PartialEq, Clone)]
pub struct MultiMap<K, V>
where
    K: Ord + PartialEq + Clone,
    V: Ord + PartialEq + Clone,
{
    map: BTreeMap<K, BTreeSet<V>>,
    empty: BTreeSet<V>,
}

impl<K, V> MultiMap<K, V>
where
    K: Ord + PartialEq + Clone,
    V: Ord + PartialEq + Clone,
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

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub(crate) fn raw_map(&self) -> &BTreeMap<K, BTreeSet<V>> {
        &self.map
    }

    pub fn consume(self) -> BTreeMap<K, BTreeSet<V>> {
        self.map
    }
}

#[derive(Clone)]
pub struct Counter<T: Ord + PartialEq + Clone> {
    map: BTreeMap<T, usize>,
    sum: usize,
}

impl<T: Ord + PartialEq + Clone> Default for Counter<T> {
    fn default() -> Counter<T> {
        Counter::new()
    }
}

impl<T: Ord + PartialEq + Clone> Counter<T> {
    pub fn new() -> Counter<T> {
        Counter {
            map: BTreeMap::new(),
            sum: 0,
        }
    }

    pub fn add(&mut self, val: T, amount: usize) -> usize {
        let entry = self.map.entry(val).or_insert(0);
        *entry += amount;
        self.sum += amount;
        *entry
    }
    pub fn inc(&mut self, val: T) -> usize {
        self.add(val, 1)
    }

    pub fn get(&self, val: T) -> usize {
        self.map.get(&val).cloned().unwrap_or(0)
    }

    pub fn sorted_asc(&self) -> Vec<&T> {
        let mut list = self.map.iter().collect::<Vec<_>>();
        list.sort_by_key(|(_, cnt)| *cnt);
        list.into_iter().map(|(t, _)| t).collect()
    }

    pub fn max(&self) -> usize {
        *self.map.values().max().unwrap()
    }
    pub fn sum(&self) -> usize {
        self.sum
    }

    pub fn compare(mut self, mut other: Counter<T>) -> Vec<(T, usize, usize)> {
        for key in self.map.keys() {
            other.map.entry(key.clone()).or_insert(0);
        }
        for key in other.map.keys() {
            self.map.entry(key.clone()).or_insert(0);
        }
        self.map
            .into_iter()
            .map(|(k, cnt)| (k.clone(), cnt, other.map[&k]))
            .collect()
    }

    pub fn borrow(&self) -> &BTreeMap<T, usize> {
        &self.map
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

// Use when your key is just PartialEq, not Ord or Hash.
pub struct VecMap<K, V> {
    inner: Vec<(K, V)>,
}

impl<K: Clone + PartialEq, V> VecMap<K, V> {
    pub fn new() -> VecMap<K, V> {
        VecMap { inner: Vec::new() }
    }

    pub fn consume(self) -> Vec<(K, V)> {
        self.inner
    }

    pub fn mut_or_insert<F: Fn() -> V>(&mut self, key: K, ctor: F) -> &mut V {
        if let Some(idx) = self.inner.iter().position(|(k, _)| key == *k) {
            return &mut self.inner[idx].1;
        }
        self.inner.push((key, ctor()));
        &mut self.inner.last_mut().unwrap().1
    }
}
