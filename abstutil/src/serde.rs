use crate::MultiMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ord;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::error::Error;

pub fn to_json<T: Serialize>(obj: &T) -> String {
    serde_json::to_string_pretty(obj).unwrap()
}

pub fn to_json_terse<T: Serialize>(obj: &T) -> String {
    serde_json::to_string(obj).unwrap()
}

pub fn from_json<T: DeserializeOwned>(raw: &Vec<u8>) -> Result<T, Box<dyn Error>> {
    serde_json::from_slice(raw).map_err(|x| x.into())
}

pub fn from_binary<T: DeserializeOwned>(raw: &Vec<u8>) -> Result<T, Box<dyn Error>> {
    bincode::deserialize(raw).map_err(|x| x.into())
}

pub fn serialized_size_bytes<T: Serialize>(obj: &T) -> usize {
    bincode::serialized_size(obj).unwrap() as usize
}

// For BTreeMaps with struct keys. See https://github.com/serde-rs/json/issues/402.

pub fn serialize_btreemap<S: Serializer, K: Serialize, V: Serialize>(
    map: &BTreeMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    map.iter().collect::<Vec<(_, _)>>().serialize(s)
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

pub fn serialize_multimap<
    S: Serializer,
    K: Serialize + Eq + Ord + Clone,
    V: Serialize + Eq + Ord + Clone,
>(
    map: &MultiMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    // TODO maybe need to sort to have deterministic output
    map.raw_map().iter().collect::<Vec<(_, _)>>().serialize(s)
}

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

pub fn serialize_usize<S: Serializer>(x: &usize, s: S) -> Result<S::Ok, S::Error> {
    if let Ok(x) = u32::try_from(*x) {
        x.serialize(s)
    } else {
        Err(serde::ser::Error::custom(format!("{} can't fit in u32", x)))
    }
}

pub fn deserialize_usize<'de, D: Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    let x = <u32>::deserialize(d)?;
    Ok(x as usize)
}
