use crate::{
    ControlStopSign, ControlTrafficSignal, IntersectionID, IntersectionType, LaneID, LaneType, Map,
    RoadID, TurnID,
};
use abstutil::{retain_btreemap, retain_btreeset, Timer};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapEdits {
    pub(crate) map_name: String,
    pub edits_name: String,
    pub commands: Vec<EditCmd>,

    // Derived from commands, kept up to date by update_derived
    pub original_lts: BTreeMap<LaneID, LaneType>,
    pub reversed_lanes: BTreeSet<LaneID>,
    pub changed_intersections: BTreeSet<IntersectionID>,

    #[serde(skip_serializing, skip_deserializing)]
    pub dirty: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EditCmd {
    ChangeLaneType {
        id: LaneID,
        lt: LaneType,
        orig_lt: LaneType,
    },
    ReverseLane {
        l: LaneID,
        // New intended dst_i
        dst_i: IntersectionID,
    },
    ChangeStopSign(ControlStopSign),
    ChangeTrafficSignal(ControlTrafficSignal),
    CloseIntersection {
        id: IntersectionID,
        orig_it: IntersectionType,
    },
    UncloseIntersection(IntersectionID, IntersectionType),
}

pub struct EditEffects {
    pub changed_lanes: BTreeSet<LaneID>,
    pub changed_roads: BTreeSet<RoadID>,
    pub changed_intersections: BTreeSet<IntersectionID>,
    pub added_turns: BTreeSet<TurnID>,
    pub deleted_turns: BTreeSet<TurnID>,
}

impl MapEdits {
    pub fn new(map_name: String) -> MapEdits {
        MapEdits {
            map_name,
            // Something has to fill this out later
            edits_name: "no_edits".to_string(),
            commands: Vec::new(),

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
            changed_intersections: BTreeSet::new(),
            dirty: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.edits_name == "no_edits" && self.commands.is_empty()
    }

    pub fn load(map_name: &str, edits_name: &str, timer: &mut Timer) -> MapEdits {
        if edits_name == "no_edits" {
            return MapEdits::new(map_name.to_string());
        }
        abstutil::read_json(
            &abstutil::path1_json(map_name, abstutil::EDITS, edits_name),
            timer,
        )
        .unwrap()
    }

    // TODO Version these
    pub(crate) fn save(&mut self, map: &Map) {
        self.compress(map);

        assert!(self.dirty);
        assert_ne!(self.edits_name, "no_edits");
        abstutil::save_json_object(abstutil::EDITS, &self.map_name, &self.edits_name, self);
        self.dirty = false;
    }

    pub fn original_it(&self, i: IntersectionID) -> IntersectionType {
        for cmd in &self.commands {
            if let EditCmd::CloseIntersection { id, orig_it } = cmd {
                if *id == i {
                    return *orig_it;
                }
            }
        }
        panic!("{} isn't closed", i);
    }

    // Original lane types, reversed lanes, and all changed intersections
    pub(crate) fn update_derived(&mut self, map: &Map, timer: &mut Timer) {
        let mut orig_lts = BTreeMap::new();
        let mut reversed_lanes = BTreeSet::new();
        let mut changed_stop_signs = BTreeSet::new();
        let mut changed_traffic_signals = BTreeSet::new();
        let mut closed_intersections = BTreeSet::new();

        for cmd in &self.commands {
            match cmd {
                EditCmd::ChangeLaneType { id, orig_lt, .. } => {
                    if !orig_lts.contains_key(id) {
                        orig_lts.insert(*id, *orig_lt);
                    }
                }
                EditCmd::ReverseLane { l, .. } => {
                    if reversed_lanes.contains(l) {
                        reversed_lanes.remove(l);
                    } else {
                        reversed_lanes.insert(*l);
                    }
                }
                EditCmd::ChangeStopSign(ss) => {
                    changed_stop_signs.insert(ss.id);
                }
                EditCmd::ChangeTrafficSignal(ts) => {
                    changed_traffic_signals.insert(ts.id);
                }
                EditCmd::CloseIntersection { id, .. } => {
                    closed_intersections.insert(*id);
                }
                EditCmd::UncloseIntersection(id, _) => {
                    closed_intersections.remove(id);
                }
            }
        }

        retain_btreemap(&mut orig_lts, |l, lt| map.get_l(*l).lane_type != *lt);
        for i in &closed_intersections {
            changed_stop_signs.remove(i);
            changed_traffic_signals.remove(i);
        }
        retain_btreeset(&mut changed_stop_signs, |i| {
            &ControlStopSign::new(map, *i) != map.get_stop_sign(*i)
        });
        retain_btreeset(&mut changed_traffic_signals, |i| {
            &ControlTrafficSignal::new(map, *i, timer) != map.get_traffic_signal(*i)
        });

        self.original_lts = orig_lts;
        self.reversed_lanes = reversed_lanes;
        self.changed_intersections = closed_intersections;
        self.changed_intersections.extend(changed_stop_signs);
        self.changed_intersections.extend(changed_traffic_signals);
    }

    // Assumes update_derived has been called.
    pub(crate) fn compress(&mut self, map: &Map) {
        let orig_cmds: Vec<EditCmd> = self.commands.drain(..).collect();

        for (l, orig_lt) in &self.original_lts {
            self.commands.push(EditCmd::ChangeLaneType {
                id: *l,
                lt: map.get_l(*l).lane_type,
                orig_lt: *orig_lt,
            });
        }
        for l in &self.reversed_lanes {
            self.commands.push(EditCmd::ReverseLane {
                l: *l,
                dst_i: map.get_l(*l).dst_i,
            });
        }
        for i in &self.changed_intersections {
            match map.get_i(*i).intersection_type {
                IntersectionType::StopSign => {
                    self.commands
                        .push(EditCmd::ChangeStopSign(map.get_stop_sign(*i).clone()));
                }
                IntersectionType::TrafficSignal => {
                    self.commands.push(EditCmd::ChangeTrafficSignal(
                        map.get_traffic_signal(*i).clone(),
                    ));
                }
                IntersectionType::Construction => {
                    // We have to recover orig_it from the original list of commands. :\
                    let mut found = false;
                    for cmd in &orig_cmds {
                        match cmd {
                            EditCmd::CloseIntersection { id, .. } => {
                                if *id == *i {
                                    self.commands.push(cmd.clone());
                                    found = true;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    assert!(found);
                }
                IntersectionType::Border => unreachable!(),
            }
        }
    }
}

impl EditEffects {
    pub fn new() -> EditEffects {
        EditEffects {
            changed_lanes: BTreeSet::new(),
            changed_roads: BTreeSet::new(),
            changed_intersections: BTreeSet::new(),
            added_turns: BTreeSet::new(),
            deleted_turns: BTreeSet::new(),
        }
    }
}

impl EditCmd {
    pub fn describe(&self) -> String {
        match self {
            EditCmd::ChangeLaneType { id, lt, .. } => format!("Change {} to {:?}", id, lt),
            EditCmd::ReverseLane { l, .. } => format!("Reverse {}", l),
            EditCmd::ChangeStopSign(ss) => format!("Edit stop sign {}", ss.id),
            EditCmd::ChangeTrafficSignal(ts) => format!("Edit traffic signal {}", ts.id),
            EditCmd::CloseIntersection { id, .. } => format!("Close {}", id),
            EditCmd::UncloseIntersection(id, _) => format!("Restore {}", id),
        }
    }
}
