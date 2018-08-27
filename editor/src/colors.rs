// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use graphics::types::Color;
use rand;
use std::collections::BTreeMap;
use std::io::Error;
use strum::IntoEnumIterator;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, EnumIter, EnumString, ToString,
         PartialOrd, Ord, Clone, Copy)]
pub enum Colors {
    Background,
    Debug,
    BrightDebug,
    Broken,
    Road,
    DrivingLaneMarking,
    Parking,
    ParkingMarking,
    Sidewalk,
    SidewalkMarking,
    Crosswalk,
    StopSignMarking,
    Biking,
    BusStopMarking,

    UnchangedIntersection,
    ChangedIntersection,

    Selected,
    Turn,
    ConflictingTurn,
    Building,
    BuildingPath,
    BuildingBoundary,
    ParcelBoundary,
    RoadOrientation,
    SearchResult,
    Visited,
    Queued,
    NextQueued,
    TurnIconCircle,
    TurnIconInactive,
    ExtraShape,

    MatchClassification,
    DontMatchClassification,

    TurnIrrelevant,
    SignalEditorTurnInCurrentCycle,
    SignalEditorTurnCompatibleWithCurrentCycle,
    SignalEditorTurnConflictsWithCurrentCycle,

    PriorityTurn,
    YieldTurn,
    StopTurn,

    DebugCar,
    MovingCar,
    StuckCar,
    ParkedCar,

    Pedestrian,

    TrafficSignalBox,
    TrafficSignalGreen,
    TrafficSignalYellow,
    TrafficSignalRed,

    StopSignBackground,
}

#[derive(Serialize, Deserialize)]
pub struct ColorScheme {
    map: BTreeMap<Colors, Color>,
}

impl ColorScheme {
    pub fn load(path: &str) -> Result<ColorScheme, Error> {
        let mut scheme: ColorScheme = abstutil::read_json(path)?;

        for color in Colors::iter() {
            if !scheme.map.contains_key(&color) {
                println!(
                    "No color for {:?} defined, initializing with a random one",
                    color
                );
                scheme
                    .map
                    .insert(color, [rand::random(), rand::random(), rand::random(), 1.0]);
            }
        }

        Ok(scheme)
    }

    pub fn get(&self, c: Colors) -> Color {
        // TODO make sure this isn't slow; maybe back this with an array
        *self.map.get(&c).unwrap()
    }

    pub fn set(&mut self, c: Colors, value: Color) {
        self.map.insert(c, value);
    }
}
