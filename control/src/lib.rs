// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate dimensioned;
extern crate map_model;
#[macro_use]
extern crate serde_derive;

use map_model::{IntersectionID, Map};
use std::collections::{BTreeMap, HashMap};
pub use stop_signs::ControlStopSign;
pub use traffic_signals::ControlTrafficSignal;

pub mod stop_signs;
pub mod traffic_signals;

// TODO awful name
pub struct ControlMap {
    pub traffic_signals: HashMap<IntersectionID, ControlTrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, ControlStopSign>,
}

impl ControlMap {
    pub fn new(
        map: &Map,
        stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
        traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,
    ) -> ControlMap {
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

        for (i, s) in stop_signs.into_iter() {
            ctrl.stop_signs.insert(i, s);
        }
        for (i, s) in traffic_signals.into_iter() {
            ctrl.traffic_signals.insert(i, s);
        }

        ctrl
    }

    pub fn get_changed_stop_signs(&self) -> BTreeMap<IntersectionID, ControlStopSign> {
        let mut h: BTreeMap<IntersectionID, ControlStopSign> = BTreeMap::new();
        for (i, s) in &self.stop_signs {
            if s.is_changed() {
                h.insert(*i, s.clone());
            }
        }
        h
    }

    pub fn get_changed_traffic_signals(&self) -> BTreeMap<IntersectionID, ControlTrafficSignal> {
        let mut h = BTreeMap::new();
        for (i, s) in &self.traffic_signals {
            if s.is_changed() {
                h.insert(*i, s.clone());
            }
        }
        h
    }
}
