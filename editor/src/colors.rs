// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate serde_json;

use graphics::types::Color;
use rand;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use strum::IntoEnumIterator;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, EnumIter, Hash)]
pub enum ColorSetting {
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
    map: HashMap<ColorSetting, Color>,
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

        for setting in ColorSetting::iter() {
            if !scheme.map.contains_key(&setting) {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("no color for {:?} defined", setting),
                ));
            }
        }

        Ok(scheme)
    }

    pub fn random_settings() -> ColorScheme {
        let mut scheme = ColorScheme {
            map: HashMap::new(),
        };
        for setting in ColorSetting::iter() {
            scheme.map.insert(
                setting,
                [rand::random(), rand::random(), rand::random(), 1.0],
            );
        }
        scheme
    }
}
