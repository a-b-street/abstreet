// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate control;
extern crate dimensioned;
extern crate ezgui;
extern crate geom;
extern crate graphics;
extern crate map_model;
extern crate multimap;
extern crate ordered_float;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate vecmath;

pub mod common;
mod straw_intersections;
pub mod straw_model;

pub use common::CarID;

// Add support for using serde with dimensioned. Won't need these hacks when
// https://github.com/paholg/dimensioned/pull/32 is merged.

use dimensioned::si;
use serde::{Deserializer, Serializer};
use serde::de;
use serde::ser::SerializeMap;
use std::collections::HashMap;
use std::fmt;

fn serialize_s<S>(secs: &si::Second<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(secs.value_unsafe)
}

struct SecondsVisitor;

impl<'de> de::Visitor<'de> for SecondsVisitor {
    type Value = f64;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an f64")
    }

    fn visit_f64<E>(self, value: f64) -> Result<f64, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}

fn deserialize_s<'de, D>(deserializer: D) -> Result<si::Second<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer
        .deserialize_f64(SecondsVisitor)
        .map(|x| x * si::S)
}

fn serialize_car_to_s_map<S>(
    map: &HashMap<CarID, si::Second<f64>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut copy = serializer.serialize_map(Some(map.len()))?;
    for (k, v) in map {
        copy.serialize_entry(k, &v.value_unsafe)?;
    }
    copy.end()
}

struct CarToSecondsMapVisitor;

impl<'de> de::Visitor<'de> for CarToSecondsMapVisitor {
    type Value = HashMap<CarID, f64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map from usize CarID to f64 seconds")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: de::MapAccess<'de>,
    {
        let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));

        while let Some((key, value)) = access.next_entry()? {
            map.insert(key, value);
        }

        Ok(map)
    }
}

fn deserialize_car_to_s_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<CarID, si::Second<f64>>, D::Error>
where
    D: Deserializer<'de>,
{
    // TODO ideally, don't copy :(
    let raw_map = deserializer.deserialize_map(CarToSecondsMapVisitor)?;
    let mut map = HashMap::with_capacity(raw_map.len());
    for (k, v) in raw_map {
        map.insert(k, v * si::S);
    }
    Ok(map)
}
