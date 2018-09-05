// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate dimensioned;
extern crate map_model;
#[macro_use]
extern crate serde_derive;

use abstutil::{deserialize_btreemap, serialize_btreemap};
use map_model::{IntersectionID, Map, TurnID};
use std::collections::{BTreeMap, HashMap};
use stop_signs::{ControlStopSign, TurnPriority};
use traffic_signals::ControlTrafficSignal;

pub mod stop_signs;
pub mod traffic_signals;

// TODO awful name
pub struct ControlMap {
    pub traffic_signals: HashMap<IntersectionID, ControlTrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, ControlStopSign>,
}

impl ControlMap {
    pub fn new(map: &Map) -> ControlMap {
        let mut ctrl = ControlMap {
            traffic_signals: HashMap::new(),
            stop_signs: HashMap::new(),
        };

        for i in map.all_intersections() {
            if i.has_traffic_signal {
                ctrl.traffic_signals
                    .insert(i.id, ControlTrafficSignal::new(map, i.id));
            } else {
                ctrl.stop_signs
                    .insert(i.id, ControlStopSign::new(map, i.id));
            }
        }

        ctrl
    }

    pub fn get_traffic_signals_savestate(&self) -> HashMap<IntersectionID, ModifiedTrafficSignal> {
        let mut h = HashMap::new();
        for (i, s) in &self.traffic_signals {
            if let Some(state) = s.get_savestate() {
                h.insert(*i, state);
            }
        }
        h
    }

    pub fn get_stop_signs_savestate(&self) -> HashMap<IntersectionID, ModifiedStopSign> {
        let mut h = HashMap::new();
        for (i, s) in &self.stop_signs {
            if let Some(state) = s.get_savestate() {
                h.insert(*i, state);
            }
        }
        h
    }

    pub fn load_savestate(
        &mut self,
        traffic_signals: &HashMap<IntersectionID, ModifiedTrafficSignal>,
        stop_signs: &HashMap<IntersectionID, ModifiedStopSign>,
    ) {
        for (i, s) in traffic_signals {
            self.traffic_signals.get_mut(i).unwrap().load_savestate(s);
        }
        for (i, s) in stop_signs {
            self.stop_signs.get_mut(i).unwrap().load_savestate(s);
        }
    }
}

// General problem: TurnIDs change as code does. Serialized state is kinda tied to code version.
// TODO diffs are happening differently for roads

#[derive(Serialize, Deserialize, Debug)]
pub struct ModifiedTrafficSignal {
    pub cycles: Vec<CycleState>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CycleState {
    pub turns: Vec<TurnID>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModifiedStopSign {
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    pub turns: BTreeMap<TurnID, TurnPriority>,
}
