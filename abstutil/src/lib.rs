extern crate multimap;
extern crate serde;
extern crate serde_cbor;
extern crate serde_json;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::Hash;
use std::io::{Error, ErrorKind, Read, Write};

pub fn to_json<T: Serialize>(obj: &T) -> String {
    serde_json::to_string_pretty(obj).unwrap()
}

pub fn write_json<T: Serialize>(path: &str, obj: &T) -> Result<(), Error> {
    let mut file = File::create(path)?;
    file.write_all(to_json(obj).as_bytes())?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let obj: T = serde_json::from_str(&contents).unwrap();
    Ok(obj)
}

pub fn write_binary<T: Serialize>(path: &str, obj: &T) -> Result<(), Error> {
    let mut file = File::create(path)?;
    serde_cbor::to_writer(&mut file, obj).map_err(|err| Error::new(ErrorKind::Other, err))
}

pub fn read_binary<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let file = File::open(path)?;
    let obj: T = serde_cbor::from_reader(file).map_err(|err| Error::new(ErrorKind::Other, err))?;
    Ok(obj)
}

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
        self.map.entry(key).or_insert(HashSet::new()).insert(value);
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

// For BTreeMaps with struct keys. See https://github.com/serde-rs/json/issues/402.

pub fn serialize_btreemap<S: Serializer, K: Serialize, V: Serialize>(
    map: &BTreeMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    map.iter()
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect::<Vec<(_, _)>>()
        .serialize(s)
}

pub fn deserialize_btreemap<
    'de,
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
>(
    d: D,
) -> Result<BTreeMap<K, V>, D::Error> {
    let vec = <Vec<(K, V)>>::deserialize(d)?;
    let mut map = BTreeMap::new();
    for (k, v) in vec {
        map.insert(k, v);
    }
    Ok(map)
}

pub fn serialize_multimap<S: Serializer, K: Serialize + Eq + Hash, V: Serialize + Eq + Hash>(
    map: &multimap::MultiMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    // TODO maybe need to sort to have deterministic output
    map.iter_all()
        .map(|(key, values)| (key.clone(), values.clone()))
        .collect::<Vec<(_, _)>>()
        .serialize(s)
}

pub fn deserialize_multimap<
    'de,
    D: Deserializer<'de>,
    K: Deserialize<'de> + Eq + Hash + Clone,
    V: Deserialize<'de> + Eq + Hash,
>(
    d: D,
) -> Result<multimap::MultiMap<K, V>, D::Error> {
    let vec = <Vec<(K, Vec<V>)>>::deserialize(d)?;
    let mut map = multimap::MultiMap::new();
    for (key, values) in vec {
        for value in values {
            map.insert(key.clone(), value);
        }
    }
    Ok(map)
}
