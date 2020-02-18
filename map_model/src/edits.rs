use crate::{
    ControlStopSign, ControlTrafficSignal, IntersectionID, LaneID, LaneType, Map, RoadID, TurnID,
};
use abstutil::{retain_btreemap, Timer};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapEdits {
    pub map_name: String,
    pub edits_name: String,
    pub commands: Vec<EditCmd>,

    // Derived from commands, kept up to date by update_derived
    pub original_lts: BTreeMap<LaneID, LaneType>,
    pub reversed_lanes: BTreeSet<LaneID>,
    pub original_intersections: BTreeMap<IntersectionID, EditIntersection>,

    // Edits without these are player generated.
    pub proposal_description: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum EditIntersection {
    StopSign(ControlStopSign),
    TrafficSignal(ControlTrafficSignal),
    Closed,
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
    ChangeIntersection {
        i: IntersectionID,
        new: EditIntersection,
        old: EditIntersection,
    },
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
            edits_name: "untitled edits".to_string(),
            proposal_description: Vec::new(),
            commands: Vec::new(),

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
        }
    }

    pub fn load(map_name: &str, edits_name: &str, timer: &mut Timer) -> MapEdits {
        if edits_name == "untitled edits" {
            return MapEdits::new(map_name.to_string());
        }
        abstutil::read_json(abstutil::path_edits(map_name, edits_name), timer)
    }

    // TODO Version these? Or it's unnecessary, since we have a command stack.
    pub(crate) fn save(&mut self, map: &Map) {
        self.compress(map);

        assert_ne!(self.edits_name, "untitled edits");
        abstutil::write_json(abstutil::path_edits(&self.map_name, &self.edits_name), self);
    }

    // Original lane types, reversed lanes, and all changed intersections
    pub(crate) fn update_derived(&mut self, map: &Map) {
        let mut orig_lts = BTreeMap::new();
        let mut reversed_lanes = BTreeSet::new();
        let mut orig_intersections: BTreeMap<IntersectionID, EditIntersection> = BTreeMap::new();

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
                EditCmd::ChangeIntersection { i, ref old, .. } => {
                    if !orig_intersections.contains_key(i) {
                        orig_intersections.insert(*i, old.clone());
                    }
                }
            }
        }

        retain_btreemap(&mut orig_lts, |l, lt| map.get_l(*l).lane_type != *lt);
        retain_btreemap(&mut orig_intersections, |i, orig| {
            map.get_i_edit(*i) != orig.clone()
        });

        self.original_lts = orig_lts;
        self.reversed_lanes = reversed_lanes;
        self.original_intersections = orig_intersections;
    }

    // Assumes update_derived has been called.
    pub(crate) fn compress(&mut self, map: &Map) {
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
        for (i, old) in &self.original_intersections {
            self.commands.push(EditCmd::ChangeIntersection {
                i: *i,
                old: old.clone(),
                new: map.get_i_edit(*i),
            });
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
