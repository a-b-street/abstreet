use crate::edits::{EditCmd, EditIntersection, EditRoad, MapEdits};
use crate::raw::OriginalRoad;
use crate::{
    osm, AccessRestrictions, ControlStopSign, Direction, IntersectionID, LaneID, LaneType, Map,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Speed, Time};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// These use permanent IDs that have a better chance of surviving basemap updates over time.

#[derive(Serialize, Deserialize, Clone)]
pub struct PermanentMapEdits {
    pub map_name: String,
    pub edits_name: String,
    pub version: usize,
    commands: Vec<PermanentEditCmd>,

    // Edits without these are player generated.
    pub proposal_description: Vec<String>,
    // The link is optional even for proposals
    pub proposal_link: Option<String>,
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

// TODO Deprecated
// Enough data to notice when lanes along a road have changed
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OriginalLane {
    pub parent: OriginalRoad,
    pub num_fwd: usize,
    pub num_back: usize,
    pub dir: Direction,
    pub idx: usize,
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
        dst_i: osm::NodeID,
    },
    ChangeSpeedLimit {
        id: OriginalRoad,
        new: Speed,
        old: Speed,
    },
    ChangeRoad {
        r: OriginalRoad,
        new: EditRoad,
        old: EditRoad,
    },
    ChangeIntersection {
        i: osm::NodeID,
        new: PermanentEditIntersection,
        old: PermanentEditIntersection,
    },
    ChangeAccessRestrictions {
        id: OriginalRoad,
        new: AccessRestrictions,
        old: AccessRestrictions,
    },
    ChangeRouteSchedule {
        osm_rel_id: osm::RelationID,
        old: Vec<Time>,
        new: Vec<Time>,
    },
}

impl PermanentMapEdits {
    pub fn to_permanent(edits: &MapEdits, map: &Map) -> PermanentMapEdits {
        PermanentMapEdits {
            map_name: map.get_name().to_string(),
            edits_name: edits.edits_name.clone(),
            // Increase this every time there's a schema change
            version: 1,
            proposal_description: edits.proposal_description.clone(),
            proposal_link: edits.proposal_link.clone(),
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
                    EditCmd::ChangeRoad { r, new, old } => PermanentEditCmd::ChangeRoad {
                        r: map.get_r(*r).orig_id,
                        new: new.clone(),
                        old: old.clone(),
                    },
                    EditCmd::ChangeIntersection { i, new, old } => {
                        PermanentEditCmd::ChangeIntersection {
                            i: map.get_i(*i).orig_id,
                            new: new.to_permanent(map),
                            old: old.to_permanent(map),
                        }
                    }
                    EditCmd::ChangeAccessRestrictions { id, new, old } => {
                        PermanentEditCmd::ChangeAccessRestrictions {
                            id: map.get_r(*id).orig_id,
                            new: new.clone(),
                            old: old.clone(),
                        }
                    }
                    EditCmd::ChangeRouteSchedule { id, old, new } => {
                        PermanentEditCmd::ChangeRouteSchedule {
                            osm_rel_id: map.get_br(*id).osm_rel_id,
                            old: old.clone(),
                            new: new.clone(),
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
            proposal_link: perma.proposal_link,
            commands: perma
                .commands
                .into_iter()
                .map(|cmd| match cmd {
                    PermanentEditCmd::ChangeLaneType { id, lt, orig_lt } => {
                        let l = id.clone().from_permanent(map)?;
                        // This validation doesn't need previous commands to be applied, because
                        // compress() creates only one ChangeLaneType per lane.
                        let now = map.get_l(l).lane_type;
                        if now != orig_lt {
                            return Err(format!(
                                "basemap lanetype of {:?} has changed from {:?} to {:?}",
                                id, orig_lt, now
                            ));
                        }
                        Ok(EditCmd::ChangeLaneType { id: l, lt, orig_lt })
                    }
                    PermanentEditCmd::ReverseLane { l, dst_i } => {
                        let l = l.from_permanent(map)?;
                        let dst_i = map.find_i_by_osm_id(dst_i)?;
                        Ok(EditCmd::ReverseLane { l, dst_i })
                    }
                    PermanentEditCmd::ChangeSpeedLimit { id, new, old } => {
                        let id = map.find_r_by_osm_id(id)?;
                        Ok(EditCmd::ChangeSpeedLimit { id, new, old })
                    }
                    PermanentEditCmd::ChangeRoad { r, new, old } => {
                        let id = map.find_r_by_osm_id(r)?;
                        let num_current = map.get_r(id).lanes_ltr().len();
                        if num_current != new.lanes_ltr.len() {
                            return Err(format!(
                                "number of lanes in {} is {} now, but {} in the edits",
                                r,
                                num_current,
                                new.lanes_ltr.len()
                            ));
                        }
                        Ok(EditCmd::ChangeRoad { r: id, new, old })
                    }
                    PermanentEditCmd::ChangeIntersection { i, new, old } => {
                        let id = map.find_i_by_osm_id(i)?;
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
                    PermanentEditCmd::ChangeAccessRestrictions { id, new, old } => {
                        let id = map.find_r_by_osm_id(id)?;
                        Ok(EditCmd::ChangeAccessRestrictions { id, new, old })
                    }
                    PermanentEditCmd::ChangeRouteSchedule {
                        osm_rel_id,
                        old,
                        new,
                    } => {
                        let id = map
                            .find_br(osm_rel_id)
                            .ok_or(format!("can't find {}", osm_rel_id))?;
                        Ok(EditCmd::ChangeRouteSchedule { id, old, new })
                    }
                })
                .collect::<Result<Vec<EditCmd>, String>>()?,

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
            original_roads: BTreeMap::new(),
            original_intersections: BTreeMap::new(),
            changed_speed_limits: BTreeSet::new(),
            changed_access_restrictions: BTreeSet::new(),
            changed_routes: BTreeSet::new(),
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
            EditIntersection::TrafficSignal(ref raw_ts) => {
                PermanentEditIntersection::TrafficSignal(raw_ts.clone())
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
                    translated_must_stop.insert(map.find_r_by_osm_id(r).ok()?, stop);
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
            PermanentEditIntersection::TrafficSignal(ts) => {
                Some(EditIntersection::TrafficSignal(ts))
            }
            PermanentEditIntersection::Closed => Some(EditIntersection::Closed),
        }
    }
}

impl OriginalLane {
    pub fn to_permanent(l: LaneID, map: &Map) -> OriginalLane {
        let r = map.get_parent(l);
        let (dir, idx) = r.dir_and_offset(l);
        OriginalLane {
            parent: r.orig_id,
            num_fwd: r.children_forwards().len(),
            num_back: r.children_backwards().len(),
            dir,
            idx,
        }
    }

    // TODO Will fail unless we apply ReverseLane's as we convert PermanentMapEdits.
    // - Could make an indexing scheme that refers to lanes from one side or the other and ignores
    //   fwd/back.
    // - Some validation happens in the lane editor, not even here.
    // - Is it inevitable? Maybe we need to apply edits as we convert.
    pub fn from_permanent(self, map: &Map) -> Result<LaneID, String> {
        let r = map.get_r(map.find_r_by_osm_id(self.parent)?);
        if r.children_forwards().len() != self.num_fwd
            || r.children_backwards().len() != self.num_back
        {
            return Err(format!(
                "number of lanes has changed in {:?} to {} fwd, {} back",
                self,
                r.children_forwards().len(),
                r.children_backwards().len()
            ));
        }
        Ok(r.children(self.dir)[self.idx].0)
    }
}
