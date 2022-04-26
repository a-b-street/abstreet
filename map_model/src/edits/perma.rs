use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Time;

use crate::edits::{EditCmd, EditCrosswalks, EditIntersection, EditRoad, MapEdits};
use crate::raw::OriginalRoad;
use crate::{osm, ControlStopSign, IntersectionID, Map, MovementID, TurnType};

/// MapEdits are converted to this before serializing. Referencing things like LaneID in a Map won't
/// work if the basemap is rebuilt from new OSM data, so instead we use stabler OSM IDs that're less
/// likely to change.
#[derive(Serialize, Deserialize, Clone)]
pub struct PermanentMapEdits {
    pub map_name: MapName,
    pub edits_name: String,
    pub version: usize,
    commands: Vec<PermanentEditCmd>,
    /// If false, adjacent roads with the same AccessRestrictions will not be merged into the same
    /// Zone; every Road will be its own Zone. This is used to experiment with a per-road cap. Note
    /// this is a map-wide setting.
    merge_zones: bool,

    /// Edits without these are player generated.
    pub proposal_description: Vec<String>,
    /// The link is optional even for proposals
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
    TrafficSignal(traffic_signal_data::TrafficSignal),
    Closed,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PermanentEditCrosswalks {
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    turns: BTreeMap<traffic_signal_data::Turn, TurnType>,
}

#[allow(clippy::enum_variant_names)]
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
    ChangeCrosswalks {
        i: osm::NodeID,
        new: PermanentEditCrosswalks,
        old: PermanentEditCrosswalks,
    },
    ChangeRouteSchedule {
        gtfs_id: String,
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
            EditCmd::ChangeCrosswalks { i, new, old } => PermanentEditCmd::ChangeCrosswalks {
                i: map.get_i(*i).orig_id,
                new: new.to_permanent(map),
                old: old.to_permanent(map),
            },
            EditCmd::ChangeRouteSchedule { id, old, new } => {
                PermanentEditCmd::ChangeRouteSchedule {
                    gtfs_id: map.get_tr(*id).gtfs_id.clone(),
                    old: old.clone(),
                    new: new.clone(),
                }
            }
        }
    }
}

impl PermanentEditCmd {
    pub fn into_cmd(self, map: &Map) -> Result<EditCmd> {
        match self {
            PermanentEditCmd::ChangeRoad { r, new, old } => {
                let id = map.find_r_by_osm_id(r)?;
                let num_current = map.get_r(id).lanes.len();
                // The basemap changed -- it'd be pretty hard to understand the original
                // intent of the edit.
                if num_current != old.lanes_ltr.len() {
                    bail!(
                        "number of lanes in {} is {} now, but {} in the edits",
                        r,
                        num_current,
                        old.lanes_ltr.len()
                    );
                }
                Ok(EditCmd::ChangeRoad { r: id, new, old })
            }
            PermanentEditCmd::ChangeIntersection { i, new, old } => {
                let id = map.find_i_by_osm_id(i)?;
                Ok(EditCmd::ChangeIntersection {
                    i: id,
                    new: new
                        .with_permanent(id, map)
                        .with_context(|| format!("new ChangeIntersection of {} invalid", i))?,
                    old: old
                        .with_permanent(id, map)
                        .with_context(|| format!("old ChangeIntersection of {} invalid", i))?,
                })
            }
            PermanentEditCmd::ChangeCrosswalks { i, new, old } => {
                let id = map.find_i_by_osm_id(i)?;
                Ok(EditCmd::ChangeCrosswalks {
                    i: id,
                    new: new
                        .with_permanent(id, map)
                        .with_context(|| format!("new ChangeCrosswalks of {} invalid", i))?,
                    old: old
                        .with_permanent(id, map)
                        .with_context(|| format!("old ChangeCrosswalks of {} invalid", i))?,
                })
            }
            PermanentEditCmd::ChangeRouteSchedule { gtfs_id, old, new } => {
                let id = map
                    .find_tr_by_gtfs(&gtfs_id)
                    .ok_or_else(|| anyhow!("can't find {}", gtfs_id))?;
                Ok(EditCmd::ChangeRouteSchedule { id, old, new })
            }
        }
    }
}

impl MapEdits {
    /// Encode the edits in a permanent format, referring to more-stable OSM IDs.
    pub fn to_permanent(&self, map: &Map) -> PermanentMapEdits {
        PermanentMapEdits {
            map_name: map.get_name().clone(),
            edits_name: self.edits_name.clone(),
            // Increase this every time there's a schema change
            version: 11,
            proposal_description: self.proposal_description.clone(),
            proposal_link: self.proposal_link.clone(),
            commands: self.commands.iter().map(|cmd| cmd.to_perma(map)).collect(),
            merge_zones: self.merge_zones,
        }
    }
}

impl PermanentMapEdits {
    /// Transform permanent edits to MapEdits, looking up the map IDs by the hopefully stabler OSM
    /// IDs. Validate that the basemap hasn't changed in important ways.
    pub fn into_edits(self, map: &Map) -> Result<MapEdits> {
        let mut edits = MapEdits {
            edits_name: self.edits_name,
            proposal_description: self.proposal_description,
            proposal_link: self.proposal_link,
            commands: self
                .commands
                .into_iter()
                .map(|cmd| cmd.into_cmd(map))
                .collect::<Result<Vec<EditCmd>>>()?,
            merge_zones: self.merge_zones,

            changed_roads: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
            original_crosswalks: BTreeMap::new(),
            changed_routes: BTreeSet::new(),
        };
        edits.update_derived(map);
        Ok(edits)
    }

    /// Transform permanent edits to MapEdits, looking up the map IDs by the hopefully stabler OSM
    /// IDs. Strip out commands that're broken, but log warnings.
    pub fn into_edits_permissive(self, map: &Map) -> MapEdits {
        let mut edits = MapEdits {
            edits_name: self.edits_name,
            proposal_description: self.proposal_description,
            proposal_link: self.proposal_link,
            commands: self
                .commands
                .into_iter()
                .filter_map(|cmd| match cmd.into_cmd(map) {
                    Ok(cmd) => Some(cmd),
                    Err(err) => {
                        warn!("Skipping broken command: {}", err);
                        None
                    }
                })
                .collect(),
            merge_zones: self.merge_zones,

            changed_roads: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
            original_crosswalks: BTreeMap::new(),
            changed_routes: BTreeSet::new(),
        };
        edits.update_derived(map);
        edits
    }

    /// Get the human-friendly of these edits. If they have a descrption, the first line is the
    /// title. Otherwise we use the filename.
    pub fn get_title(&self) -> &str {
        if self.proposal_description.is_empty() {
            &self.edits_name
        } else {
            &self.proposal_description[0]
        }
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
    fn with_permanent(self, i: IntersectionID, map: &Map) -> Result<EditIntersection> {
        match self {
            PermanentEditIntersection::StopSign { must_stop } => {
                let mut translated_must_stop = BTreeMap::new();
                for (r, stop) in must_stop {
                    translated_must_stop.insert(map.find_r_by_osm_id(r)?, stop);
                }

                // Make sure the roads exactly match up
                let mut ss = ControlStopSign::new(map, i);
                if translated_must_stop.len() != ss.roads.len() {
                    bail!(
                        "Stop sign has {} roads now, but {} from edits",
                        ss.roads.len(),
                        translated_must_stop.len()
                    );
                }
                for (r, stop) in translated_must_stop {
                    if let Some(road) = ss.roads.get_mut(&r) {
                        road.must_stop = stop;
                    } else {
                        bail!("{} doesn't connect to {}", i, r);
                    }
                }

                Ok(EditIntersection::StopSign(ss))
            }
            PermanentEditIntersection::TrafficSignal(ts) => Ok(EditIntersection::TrafficSignal(ts)),
            PermanentEditIntersection::Closed => Ok(EditIntersection::Closed),
        }
    }
}

impl EditCrosswalks {
    fn to_permanent(&self, map: &Map) -> PermanentEditCrosswalks {
        PermanentEditCrosswalks {
            turns: self
                .0
                .iter()
                .map(|(id, turn_type)| (id.to_movement(map).to_permanent(map), *turn_type))
                .collect(),
        }
    }
}

impl PermanentEditCrosswalks {
    fn with_permanent(self, i: IntersectionID, map: &Map) -> Result<EditCrosswalks> {
        let mut turns = BTreeMap::new();
        for (id, turn_type) in self.turns {
            let movement = MovementID::from_permanent(id, map)?;
            // Find all TurnIDs that map to this MovementID
            let mut turn_ids = Vec::new();
            for turn in &map.get_i(i).turns {
                if turn.id.to_movement(map) == movement {
                    turn_ids.push(turn.id);
                }
            }
            if turn_ids.len() != 1 {
                bail!(
                    "{:?} didn't map to exactly 1 crossing turn: {:?}",
                    movement,
                    turn_ids
                );
            }
            turns.insert(turn_ids.pop().unwrap(), turn_type);
        }
        Ok(EditCrosswalks(turns))
    }
}
