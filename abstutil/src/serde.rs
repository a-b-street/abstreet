use std::cmp::Ord;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::MultiMap;

/// Stringifies an object to nicely formatted JSON.
pub fn to_json<T: Serialize>(obj: &T) -> String {
    serde_json::to_string_pretty(obj).unwrap()
}

/// Stringifies an object to terse JSON.
pub fn to_json_terse<T: Serialize>(obj: &T) -> String {
    serde_json::to_string(obj).unwrap()
}

/// Deserializes an object from a JSON string.
pub fn from_json<T: DeserializeOwned>(raw: &Vec<u8>) -> Result<T> {
    serde_json::from_slice(raw).map_err(|err| err.into())
}

/// Deserializes an object from JSON, from a reader.
pub fn from_json_reader<R: std::io::Read, T: DeserializeOwned>(reader: R) -> Result<T> {
    serde_json::from_reader(reader).map_err(|err| err.into())
}

/// Deserializes an object from the bincode format.
pub fn from_binary<T: DeserializeOwned>(raw: &Vec<u8>) -> Result<T> {
    bincode::deserialize(raw).map_err(|err| err.into())
}

/// Deserializes an object from the bincode format, from a reader.
pub fn from_binary_reader<R: std::io::Read, T: DeserializeOwned>(reader: R) -> Result<T> {
    bincode::deserialize_from(reader).map_err(|err| err.into())
}

/// The number of bytes for an object serialized to bincode.
pub fn serialized_size_bytes<T: Serialize>(obj: &T) -> usize {
    bincode::serialized_size(obj).unwrap() as usize
}

/// Serializes a BTreeMap as a list of tuples. Necessary when the keys are structs; see
/// https://github.com/serde-rs/json/issues/402.
pub fn serialize_btreemap<S: Serializer, K: Serialize, V: Serialize>(
    map: &BTreeMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    map.iter().collect::<Vec<(_, _)>>().serialize(s)
}

/// Deserializes a BTreeMap from a list of tuples. Necessary when the keys are structs; see
/// https://github.com/serde-rs/json/issues/402.
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

/// Serializes a HashMap as a list of tuples, first sorting by the keys. This ensures the
/// serialized form is deterministic.
pub fn serialize_hashmap<S: Serializer, K: Serialize + Ord, V: Serialize>(
    map: &HashMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    let mut list: Vec<(&K, &V)> = map.iter().collect();
    list.sort_by_key(|(k, _)| k.clone());
    list.serialize(s)
}

/// Deserializes a HashMap from a list of tuples.
pub fn deserialize_hashmap<
    'de,
    D: Deserializer<'de>,
    K: Deserialize<'de> + std::hash::Hash + Eq,
    V: Deserialize<'de>,
>(
    d: D,
) -> Result<HashMap<K, V>, D::Error> {
    let vec = <Vec<(K, V)>>::deserialize(d)?;
    let mut map = HashMap::new();
    for (k, v) in vec {
        map.insert(k, v);
    }
    Ok(map)
}

/// Serializes a MultiMap.
pub fn serialize_multimap<
    S: Serializer,
    K: Serialize + Eq + Ord + Clone,
    V: Serialize + Eq + Ord + Clone,
>(
    map: &MultiMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    map.borrow().iter().collect::<Vec<(_, _)>>().serialize(s)
}

/// Deserializes a MultiMap.
pub fn deserialize_multimap<
    'de,
    D: Deserializer<'de>,
    K: Deserialize<'de> + Eq + Ord + Clone,
    V: Deserialize<'de> + Eq + Ord + Clone,
>(
    d: D,
) -> Result<MultiMap<K, V>, D::Error> {
    let vec = <Vec<(K, Vec<V>)>>::deserialize(d)?;
    let mut map = MultiMap::new();
    for (key, values) in vec {
        for value in values {
            map.insert(key.clone(), value);
        }
    }
    Ok(map)
}

/// Serializes a `usize` as a `u32` to save space. Useful when you need `usize` for indexing, but
/// the values don't exceed 2^32.
pub fn serialize_usize<S: Serializer>(x: &usize, s: S) -> Result<S::Ok, S::Error> {
    if let Ok(x) = u32::try_from(*x) {
        x.serialize(s)
    } else {
        Err(serde::ser::Error::custom(format!("{} can't fit in u32", x)))
    }
}

/// Deserializes a `usize` from a `u32`.
pub fn deserialize_usize<'de, D: Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    let x = <u32>::deserialize(d)?;
    Ok(x as usize)
}
