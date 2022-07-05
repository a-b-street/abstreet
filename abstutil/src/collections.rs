use std::cmp::Ord;
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;

use anyhow::Result;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

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

    pub fn set(&mut self, key: K, values: BTreeSet<V>) {
        self.map.insert(key, values);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[allow(clippy::should_implement_trait)]
    pub fn borrow(&self) -> &BTreeMap<K, BTreeSet<V>> {
        &self.map
    }

    pub fn consume(self) -> BTreeMap<K, BTreeSet<V>> {
        self.map
    }
}
impl<K, V> Default for MultiMap<K, V>
where
    K: Ord + PartialEq + Clone,
    V: Ord + PartialEq + Clone,
{
    fn default() -> MultiMap<K, V> {
        MultiMap::new()
    }
}

/// A counter per key
// Be careful with PartialEq -- some entries may have an explicit 0, others not
#[derive(Serialize, Deserialize, Clone, PartialEq)]
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

    /// If the key is missing, returns 0
    pub fn get(&self, val: T) -> usize {
        self.map.get(&val).cloned().unwrap_or(0)
    }

    /// Values with the same count are grouped together
    pub fn sorted_asc(&self) -> Vec<Vec<T>> {
        let mut list = self.map.iter().collect::<Vec<_>>();
        list.sort_by_key(|(_, cnt)| *cnt);
        list.into_iter()
            .group_by(|(_, cnt)| *cnt)
            .into_iter()
            .map(|(_, group)| group.into_iter().map(|(val, _)| val.clone()).collect())
            .collect()
    }

    pub fn highest_n(&self, n: usize) -> Vec<(T, usize)> {
        let mut list: Vec<(T, usize)> = self
            .map
            .iter()
            .map(|(key, cnt)| (key.clone(), *cnt))
            .collect();
        list.sort_by_key(|(_, cnt)| *cnt);
        list.reverse();
        list.truncate(n);
        list
    }

    /// If two keys share the maximum, returns one of them arbitrarily (and deterministically)
    pub fn max_key(&self) -> T {
        self.map.iter().max_by_key(|pair| pair.1).unwrap().0.clone()
    }

    pub fn max(&self) -> usize {
        self.map.values().max().cloned().unwrap_or(0)
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

    #[allow(clippy::should_implement_trait)]
    pub fn borrow(&self) -> &BTreeMap<T, usize> {
        &self.map
    }
    pub fn consume(self) -> BTreeMap<T, usize> {
        self.map
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn extend(&mut self, other: Counter<T>) {
        self.map.extend(other.map);
        self.sum += other.sum;
    }
}

pub fn wraparound_get<T>(vec: &[T], idx: isize) -> &T {
    let len = vec.len() as isize;
    let idx = idx % len;
    let idx = if idx >= 0 { idx } else { idx + len };
    &vec[idx as usize]
}

pub fn contains_duplicates<T: Ord>(vec: &[T]) -> bool {
    let mut set = BTreeSet::new();
    for item in vec {
        if set.contains(item) {
            return true;
        }
        set.insert(item);
    }
    false
}

/// Use when your key is just PartialEq, not Ord or Hash.
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

    /// Doesn't dedupe
    pub fn push(&mut self, key: K, value: V) {
        self.inner.push((key, value));
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        for (k, v) in &self.inner {
            if k == key {
                return Some(v);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<K: Clone + PartialEq, V> Default for VecMap<K, V> {
    fn default() -> Self {
        VecMap::new()
    }
}

/// Convenience functions around a string->string map
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tags(BTreeMap<String, String>);

impl Tags {
    pub fn new(map: BTreeMap<String, String>) -> Tags {
        Tags(map)
    }

    pub fn empty() -> Tags {
        Tags(BTreeMap::new())
    }

    pub fn get(&self, k: &str) -> Option<&String> {
        self.0.get(k)
    }

    pub fn get_result(&self, k: &str) -> Result<&String> {
        self.0.get(k).ok_or_else(|| anyhow!("missing {}", k))
    }

    pub fn contains_key(&self, k: &str) -> bool {
        self.0.contains_key(k)
    }
    pub fn has_any(&self, keys: Vec<&str>) -> bool {
        keys.into_iter().any(|key| self.contains_key(key))
    }

    pub fn is(&self, k: &str, v: &str) -> bool {
        self.0.get(k) == Some(&v.to_string())
    }

    pub fn is_any(&self, k: &str, values: Vec<&str>) -> bool {
        if let Some(v) = self.0.get(k) {
            values.contains(&v.as_ref())
        } else {
            false
        }
    }

    pub fn insert<K: Into<String>, V: Into<String>>(&mut self, k: K, v: V) {
        self.0.insert(k.into(), v.into());
    }
    pub fn remove(&mut self, k: &str) -> Option<String> {
        self.0.remove(k)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    // TODO Really just iter()
    pub fn inner(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    pub fn into_inner(self) -> BTreeMap<String, String> {
        self.0
    }

    /// Find all values that differ. Returns (key, value1, value2). If one set of tags is missing a
    /// value, return a blank string.
    pub fn diff(&self, other: &Tags) -> Vec<(String, String, String)> {
        let mut results = Vec::new();
        for (k, v1) in self.inner() {
            let v2 = other.get(k).cloned().unwrap_or_else(String::new);
            if v1 != &v2 {
                results.push((k.clone(), v1.clone(), v2));
            }
        }
        for (k, v2) in other.inner() {
            if !self.contains_key(k) {
                results.push((k.clone(), String::new(), v2.clone()));
            }
        }
        results
    }
}

/// Use with `FixedMap`. From a particular key, extract a `usize`. These values should be
/// roughly contiguous; the space used by the `FixedMap` will be `O(n)` with respect to the largest
/// value returned here.
pub trait IndexableKey {
    fn index(&self) -> usize;
}

/// A drop-in replacement for `BTreeMap`, where the keys have the property of being array indices.
/// Some values may be missing. Much more efficient at operations on individual objects, because
/// it just becomes a simple array lookup.
#[derive(Serialize, Deserialize, Clone)]
pub struct FixedMap<K: IndexableKey, V> {
    inner: Vec<Option<V>>,
    key_type: PhantomData<K>,
}

impl<K: IndexableKey, V> FixedMap<K, V> {
    pub fn new() -> FixedMap<K, V> {
        FixedMap {
            inner: Vec::new(),
            key_type: PhantomData,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let idx = key.index();
        // Depending on the order of calls, this could wind up pushing one value at a time. It may
        // be more efficient to resize less times and allocate more, but it'll require the caller
        // to know about how many values it'll need.
        if idx >= self.inner.len() {
            self.inner.resize_with(idx + 1, || None);
        }
        self.inner[idx] = Some(value);
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key.index())?.as_ref()
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.inner.get_mut(key.index())?.as_mut()
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.inner
            .get(key.index())
            .map(|x| x.is_some())
            .unwrap_or(false)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.get_mut(key.index())?.take()
    }

    pub fn values(&self) -> std::iter::Flatten<std::slice::Iter<'_, std::option::Option<V>>> {
        self.inner.iter().flatten()
    }
}

impl<K: IndexableKey, V> Default for FixedMap<K, V> {
    fn default() -> Self {
        FixedMap::new()
    }
}

impl<K: IndexableKey, V> std::ops::Index<&K> for FixedMap<K, V> {
    type Output = V;

    fn index(&self, key: &K) -> &Self::Output {
        self.inner[key.index()].as_ref().unwrap()
    }
}
