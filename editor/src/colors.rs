// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate serde_json;

use graphics::types::Color;
use rand;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use strum::IntoEnumIterator;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, EnumIter, PartialOrd, Ord)]
pub enum Colors {
    Debug,
    BrightDebug,
    Road,
    Parking,
    Sidewalk,
    ChangedStopSignIntersection,
    ChangedTrafficSignalIntersection,
    TrafficSignalIntersection,
    NormalIntersection,
    Selected,
    Turn,
    ConflictingTurn,
    Building,
    ParcelBoundary,
    ParcelInterior,
    RoadOrientation,
    SearchResult,
    Visited,
    Queued,
    NextQueued,
    TurnIconCircle,
    TurnIconInactive,
}

#[derive(Serialize, Deserialize)]
pub struct ColorScheme {
    map: BTreeMap<Colors, Color>,
}

impl ColorScheme {
    pub fn write(&self, path: &str) -> Result<(), Error> {
        let mut file = File::create(path)?;
        file.write_all(serde_json::to_string_pretty(self).unwrap().as_bytes())?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<ColorScheme, Error> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let scheme: ColorScheme = serde_json::from_str(&contents).unwrap();

        for color in Colors::iter() {
            if !scheme.map.contains_key(&color) {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("no color for {:?} defined", color),
                ));
            }
        }

        Ok(scheme)
    }

    pub fn random_colors() -> ColorScheme {
        let mut scheme = ColorScheme {
            map: BTreeMap::new(),
        };
        for color in Colors::iter() {
            scheme
                .map
                .insert(color, [rand::random(), rand::random(), rand::random(), 1.0]);
        }
        scheme
    }

    pub fn get(&self, c: Colors) -> Color {
        // TODO make sure this isn't slow; maybe back this with an array
        *self.map.get(&c).unwrap()
    }
}
