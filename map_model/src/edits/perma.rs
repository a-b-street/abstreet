use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Time;

use crate::edits::{EditCmd, EditIntersection, EditRoad, MapEdits};
use crate::raw::OriginalRoad;
use crate::{osm, ControlStopSign, IntersectionID, Map};

// MapEdits are converted to this before serializing. Referencing things like LaneID in a Map won't
// work if the basemap is rebuilt from new OSM data, so instead we use stabler OSM IDs that're less
// likely to change.
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
pub enum PermanentEditIntersection {
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

#[derive(Serialize, Deserialize, Clone)]
pub enum PermanentEditCmd {
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
    ChangeRouteSchedule {
        osm_rel_id: osm::RelationID,
        old: Vec<Time>,
        new: Vec<Time>,
    },
}

impl EditCmd {
    pub fn to_perma(&self, map: &Map) -> PermanentEditCmd {
        match self {
            EditCmd::ChangeRoad { r, new, old } => PermanentEditCmd::ChangeRoad {
                r: map.get_r(*r).orig_id,
                new: new.clone(),
                old: old.clone(),
            },
            EditCmd::ChangeIntersection { i, new, old } => PermanentEditCmd::ChangeIntersection {
                i: map.get_i(*i).orig_id,
                new: new.to_permanent(map),
                old: old.to_permanent(map),
            },
            EditCmd::ChangeRouteSchedule { id, old, new } => {
                PermanentEditCmd::ChangeRouteSchedule {
                    osm_rel_id: map.get_br(*id).osm_rel_id,
                    old: old.clone(),
                    new: new.clone(),
                }
            }
        }
    }
}

impl PermanentMapEdits {
    pub fn to_permanent(edits: &MapEdits, map: &Map) -> PermanentMapEdits {
        PermanentMapEdits {
            map_name: map.get_name().to_string(),
            edits_name: edits.edits_name.clone(),
            // Increase this every time there's a schema change
            version: 2,
            proposal_description: edits.proposal_description.clone(),
            proposal_link: edits.proposal_link.clone(),
            commands: edits.commands.iter().map(|cmd| cmd.to_perma(map)).collect(),
        }
    }

    // Load edits from the permanent form, looking up the Map IDs by the hopefully stabler OSM IDs.
    // Validate that the basemap hasn't changed in important ways.
    // TODO When a change has happened, try to preserve as much of the original edits as possible,
    // and warn the player about the rest?
    pub fn from_permanent(perma: PermanentMapEdits, map: &Map) -> Result<MapEdits, String> {
        let mut edits = MapEdits {
            edits_name: perma.edits_name,
            proposal_description: perma.proposal_description,
            proposal_link: perma.proposal_link,
            commands: perma
                .commands
                .into_iter()
                .map(|cmd| match cmd {
                    PermanentEditCmd::ChangeRoad { r, new, old } => {
                        let id = map.find_r_by_osm_id(r)?;
                        let num_current = map.get_r(id).lanes_ltr().len();
                        // The basemap changed -- it'd be pretty hard to understand the original
                        // intent of the edit.
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
                            new: new.from_permanent(id, map).map_err(|err| {
                                format!("new ChangeIntersection of {} invalid: {}", i, err)
                            })?,
                            old: old.from_permanent(id, map).map_err(|err| {
                                format!("old ChangeIntersection of {} invalid: {}", i, err)
                            })?,
                        })
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

            changed_roads: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
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
    fn from_permanent(self, i: IntersectionID, map: &Map) -> Result<EditIntersection, String> {
        match self {
            PermanentEditIntersection::StopSign { must_stop } => {
                let mut translated_must_stop = BTreeMap::new();
                for (r, stop) in must_stop {
                    translated_must_stop.insert(map.find_r_by_osm_id(r)?, stop);
                }

                // Make sure the roads exactly match up
                let mut ss = ControlStopSign::new(map, i);
                if translated_must_stop.len() != ss.roads.len() {
                    return Err(format!(
                        "Stop sign has {} roads now, but {} from edits",
                        ss.roads.len(),
                        translated_must_stop.len()
                    ));
                }
                for (r, stop) in translated_must_stop {
                    if let Some(road) = ss.roads.get_mut(&r) {
                        road.must_stop = stop;
                    } else {
                        return Err(format!("{} doesn't connect to {}", i, r));
                    }
                }

                Ok(EditIntersection::StopSign(ss))
            }
            PermanentEditIntersection::TrafficSignal(ts) => Ok(EditIntersection::TrafficSignal(ts)),
            PermanentEditIntersection::Closed => Ok(EditIntersection::Closed),
        }
    }
}
