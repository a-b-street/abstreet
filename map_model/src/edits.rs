use crate::raw::{OriginalIntersection, OriginalRoad};
use crate::{
    ControlStopSign, ControlTrafficSignal, IntersectionID, LaneID, LaneType, Map, RoadID, TurnID,
};
use abstutil::{deserialize_btreemap, retain_btreemap, retain_btreeset, serialize_btreemap, Timer};
use geom::Speed;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct MapEdits {
    pub edits_name: String,
    pub commands: Vec<EditCmd>,

    // Derived from commands, kept up to date by update_derived
    pub original_lts: BTreeMap<LaneID, LaneType>,
    pub reversed_lanes: BTreeSet<LaneID>,
    pub original_intersections: BTreeMap<IntersectionID, EditIntersection>,
    pub changed_speed_limits: BTreeSet<RoadID>,

    // Edits without these are player generated.
    pub proposal_description: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditIntersection {
    StopSign(ControlStopSign),
    TrafficSignal(ControlTrafficSignal),
    Closed,
}

#[derive(Debug, Clone)]
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
    ChangeSpeedLimit {
        id: RoadID,
        new: Speed,
        old: Speed,
    },
    ChangeIntersection {
        i: IntersectionID,
        new: EditIntersection,
        old: EditIntersection,
    },
}

pub struct EditEffects {
    pub changed_roads: BTreeSet<RoadID>,
    pub changed_intersections: BTreeSet<IntersectionID>,
    pub added_turns: BTreeSet<TurnID>,
    pub deleted_turns: BTreeSet<TurnID>,
}

impl MapEdits {
    pub fn new() -> MapEdits {
        MapEdits {
            // Something has to fill this out later
            edits_name: "untitled edits".to_string(),
            proposal_description: Vec::new(),
            commands: Vec::new(),

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
            changed_speed_limits: BTreeSet::new(),
        }
    }

    pub fn load(map: &Map, edits_name: &str, timer: &mut Timer) -> Result<MapEdits, String> {
        if edits_name == "untitled edits" {
            return Ok(MapEdits::new());
        }
        PermanentMapEdits::from_permanent(
            abstutil::read_json(abstutil::path_edits(map.get_name(), edits_name), timer),
            map,
        )
    }

    // TODO Version these? Or it's unnecessary, since we have a command stack.
    pub(crate) fn save(&self, map: &Map) {
        assert_ne!(self.edits_name, "untitled edits");

        abstutil::write_json(
            abstutil::path_edits(map.get_name(), &self.edits_name),
            &PermanentMapEdits::to_permanent(self, map),
        );
    }

    pub(crate) fn update_derived(&mut self, map: &Map) {
        let mut orig_lts = BTreeMap::new();
        let mut reversed_lanes = BTreeSet::new();
        let mut orig_intersections: BTreeMap<IntersectionID, EditIntersection> = BTreeMap::new();
        let mut changed_speed_limits = BTreeSet::new();

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
                EditCmd::ChangeSpeedLimit { id, .. } => {
                    changed_speed_limits.insert(*id);
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
        retain_btreeset(&mut changed_speed_limits, |r| {
            map.get_r(*r).speed_limit != map.get_r(*r).speed_limit_from_osm()
        });

        self.original_lts = orig_lts;
        self.reversed_lanes = reversed_lanes;
        self.original_intersections = orig_intersections;
        self.changed_speed_limits = changed_speed_limits;
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
        for r in &self.changed_speed_limits {
            self.commands.push(EditCmd::ChangeSpeedLimit {
                id: *r,
                new: map.get_r(*r).speed_limit,
                old: map.get_r(*r).speed_limit_from_osm(),
            });
        }
    }
}

impl std::default::Default for MapEdits {
    fn default() -> MapEdits {
        MapEdits::new()
    }
}

impl EditEffects {
    pub fn new() -> EditEffects {
        EditEffects {
            changed_roads: BTreeSet::new(),
            changed_intersections: BTreeSet::new(),
            added_turns: BTreeSet::new(),
            deleted_turns: BTreeSet::new(),
        }
    }
}

// These mirror the above, except they use permanent IDs that have a better chance of surviving
// basemap updates over time.

#[derive(Serialize, Deserialize, Clone)]
pub struct PermanentMapEdits {
    pub map_name: String,
    pub edits_name: String,
    commands: Vec<PermanentEditCmd>,

    // Edits without these are player generated.
    pub proposal_description: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
enum PermanentEditIntersection {
    StopSign {
        #[serde(
            serialize_with = "serialize_btreemap",
            deserialize_with = "deserialize_btreemap"
        )]
        must_stop: BTreeMap<OriginalRoad, bool>,
    },
    TrafficSignal(seattle_traffic_signals::TrafficSignal),
    Closed,
}

// Enough data to notice when lanes along a road have changed
#[derive(Serialize, Deserialize, Clone, Debug)]
struct OriginalLane {
    parent: OriginalRoad,
    num_fwd: usize,
    num_back: usize,
    fwd: bool,
    idx: usize,
}

#[derive(Serialize, Deserialize, Clone)]
enum PermanentEditCmd {
    ChangeLaneType {
        id: OriginalLane,
        lt: LaneType,
        orig_lt: LaneType,
    },
    ReverseLane {
        l: OriginalLane,
        // New intended dst_i
        dst_i: OriginalIntersection,
    },
    ChangeSpeedLimit {
        id: OriginalRoad,
        new: Speed,
        old: Speed,
    },
    ChangeIntersection {
        i: OriginalIntersection,
        new: PermanentEditIntersection,
        old: PermanentEditIntersection,
    },
}

impl PermanentMapEdits {
    fn to_permanent(edits: &MapEdits, map: &Map) -> PermanentMapEdits {
        PermanentMapEdits {
            map_name: map.get_name().to_string(),
            edits_name: edits.edits_name.clone(),
            proposal_description: edits.proposal_description.clone(),
            commands: edits
                .commands
                .iter()
                .map(|cmd| match cmd {
                    EditCmd::ChangeLaneType { id, lt, orig_lt } => {
                        PermanentEditCmd::ChangeLaneType {
                            id: OriginalLane::to_permanent(*id, map),
                            lt: *lt,
                            orig_lt: *orig_lt,
                        }
                    }
                    EditCmd::ReverseLane { l, dst_i } => PermanentEditCmd::ReverseLane {
                        l: OriginalLane::to_permanent(*l, map),
                        dst_i: map.get_i(*dst_i).orig_id,
                    },
                    EditCmd::ChangeSpeedLimit { id, new, old } => {
                        PermanentEditCmd::ChangeSpeedLimit {
                            id: map.get_r(*id).orig_id,
                            new: *new,
                            old: *old,
                        }
                    }
                    EditCmd::ChangeIntersection { i, new, old } => {
                        PermanentEditCmd::ChangeIntersection {
                            i: map.get_i(*i).orig_id,
                            new: new.to_permanent(map),
                            old: old.to_permanent(map),
                        }
                    }
                })
                .collect(),
        }
    }

    pub fn from_permanent(perma: PermanentMapEdits, map: &Map) -> Result<MapEdits, String> {
        let mut edits = MapEdits {
            edits_name: perma.edits_name,
            proposal_description: perma.proposal_description,
            commands: perma
                .commands
                .into_iter()
                .map(|cmd| match cmd {
                    PermanentEditCmd::ChangeLaneType { id, lt, orig_lt } => {
                        let l = id.clone().from_permanent(map)?;
                        if map.get_l(l).lane_type != orig_lt {
                            return Err(format!("basemap lanetype of {:?} has changed", id));
                        }
                        Ok(EditCmd::ChangeLaneType { id: l, lt, orig_lt })
                    }
                    PermanentEditCmd::ReverseLane { l, dst_i } => {
                        let l = l.from_permanent(map)?;
                        let dst_i = map.find_i_by_osm_id(dst_i.osm_node_id)?;
                        Ok(EditCmd::ReverseLane { l, dst_i })
                    }
                    PermanentEditCmd::ChangeSpeedLimit { id, new, old } => {
                        let id = map.find_r_by_osm_id(
                            id.osm_way_id,
                            (id.i1.osm_node_id, id.i2.osm_node_id),
                        )?;
                        Ok(EditCmd::ChangeSpeedLimit { id, new, old })
                    }
                    PermanentEditCmd::ChangeIntersection { i, new, old } => {
                        let id = map.find_i_by_osm_id(i.osm_node_id)?;
                        Ok(EditCmd::ChangeIntersection {
                            i: id,
                            new: new
                                .from_permanent(id, map)
                                .ok_or(format!("new ChangeIntersection of {} invalid", i))?,
                            old: old
                                .from_permanent(id, map)
                                .ok_or(format!("old ChangeIntersection of {} invalid", i))?,
                        })
                    }
                })
                .collect::<Result<Vec<EditCmd>, String>>()?,

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
            changed_speed_limits: BTreeSet::new(),
        };
        edits.update_derived(map);
        Ok(edits)
    }
}

impl EditIntersection {
    fn to_permanent(&self, map: &Map) -> PermanentEditIntersection {
        match self {
            EditIntersection::StopSign(ref ss) => PermanentEditIntersection::StopSign {
                must_stop: ss
                    .roads
                    .iter()
                    .map(|(r, val)| (map.get_r(*r).orig_id, val.must_stop))
                    .collect(),
            },
            EditIntersection::TrafficSignal(ref ts) => {
                PermanentEditIntersection::TrafficSignal(ts.export(map))
            }
            EditIntersection::Closed => PermanentEditIntersection::Closed,
        }
    }
}

impl PermanentEditIntersection {
    fn from_permanent(self, i: IntersectionID, map: &Map) -> Option<EditIntersection> {
        match self {
            PermanentEditIntersection::StopSign { must_stop } => {
                let mut translated_must_stop = BTreeMap::new();
                for (r, stop) in must_stop {
                    translated_must_stop.insert(
                        map.find_r_by_osm_id(r.osm_way_id, (r.i1.osm_node_id, r.i2.osm_node_id))
                            .ok()?,
                        stop,
                    );
                }

                // Make sure the roads exactly match up
                let mut ss = ControlStopSign::new(map, i);
                if translated_must_stop.len() != ss.roads.len() {
                    return None;
                }
                for (r, stop) in translated_must_stop {
                    ss.roads.get_mut(&r)?.must_stop = stop;
                }

                Some(EditIntersection::StopSign(ss))
            }
            PermanentEditIntersection::TrafficSignal(ts) => Some(EditIntersection::TrafficSignal(
                ControlTrafficSignal::import(ts, i, map)?,
            )),
            PermanentEditIntersection::Closed => Some(EditIntersection::Closed),
        }
    }
}

impl OriginalLane {
    fn to_permanent(l: LaneID, map: &Map) -> OriginalLane {
        let r = map.get_parent(l);
        let (fwd, idx) = r.dir_and_offset(l);
        OriginalLane {
            parent: r.orig_id,
            num_fwd: r.children_forwards.len(),
            num_back: r.children_backwards.len(),
            fwd,
            idx,
        }
    }

    fn from_permanent(self, map: &Map) -> Result<LaneID, String> {
        let r = map.get_r(map.find_r_by_osm_id(
            self.parent.osm_way_id,
            (self.parent.i1.osm_node_id, self.parent.i2.osm_node_id),
        )?);
        if r.children_forwards.len() != self.num_fwd || r.children_backwards.len() != self.num_back
        {
            return Err(format!("number of lanes has changed in {:?}", self));
        }
        if self.fwd {
            Ok(r.children_forwards[self.idx].0)
        } else {
            Ok(r.children_backwards[self.idx].0)
        }
    }
}
