//! Once a Map exists, the player can edit it in the UI (producing `MapEdits` in-memory), then save
//! the changes to a file (as `PermanentMapEdits`). See
//! <https://a-b-street.github.io/docs/tech/map/edits.html>.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::{Speed, Time};
use osm2streets::get_lane_specs_ltr;

pub use self::perma::PermanentMapEdits;
use crate::{
    AccessRestrictions, ControlStopSign, ControlTrafficSignal, IntersectionControl, IntersectionID,
    LaneID, LaneSpec, Map, MapConfig, ParkingLotID, Road, RoadID, TransitRouteID, TurnID, TurnType,
};

mod apply;
mod compat;
mod perma;

/// Represents changes to a map. Note this isn't serializable -- that's what `PermanentMapEdits`
/// does.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEdits {
    pub edits_name: String,
    /// A stack, oldest edit is first. The same intersection may be edited multiple times in this
    /// stack, until compress() happens.
    pub commands: Vec<EditCmd>,
    /// If false, adjacent roads with the same AccessRestrictions will not be merged into the same
    /// Zone; every Road will be its own Zone. This is used to experiment with a per-road cap. Note
    /// this is a map-wide setting.
    pub merge_zones: bool,

    /// Derived from commands, kept up to date by update_derived
    pub changed_roads: BTreeSet<RoadID>,
    pub original_intersections: BTreeMap<IntersectionID, EditIntersection>,
    pub original_crosswalks: BTreeMap<IntersectionID, EditCrosswalks>,
    pub changed_routes: BTreeSet<TransitRouteID>,

    /// Some edits are included in the game by default, in data/system/proposals, as "community
    /// proposals." They require a description and may have a link to a write-up.
    pub proposal_description: Vec<String>,
    pub proposal_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditCmd {
    ChangeRoad {
        r: RoadID,
        old: EditRoad,
        new: EditRoad,
    },
    ChangeIntersection {
        i: IntersectionID,
        new: EditIntersection,
        old: EditIntersection,
    },
    ChangeRouteSchedule {
        id: TransitRouteID,
        old: Vec<Time>,
        new: Vec<Time>,
    },
    ChangeCrosswalks {
        i: IntersectionID,
        old: EditCrosswalks,
        new: EditCrosswalks,
    },
}

pub struct EditEffects {
    pub changed_roads: BTreeSet<RoadID>,
    pub deleted_lanes: BTreeSet<LaneID>,
    pub changed_intersections: BTreeSet<IntersectionID>,
    // TODO Will we need modified turns?
    pub added_turns: BTreeSet<TurnID>,
    pub deleted_turns: BTreeSet<TurnID>,
    pub changed_parking_lots: BTreeSet<ParkingLotID>,
    modified_lanes: BTreeSet<LaneID>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditRoad {
    pub lanes_ltr: Vec<LaneSpec>,
    pub speed_limit: Speed,
    pub access_restrictions: AccessRestrictions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditIntersection {
    StopSign(ControlStopSign),
    // Don't keep ControlTrafficSignal here, because it contains movements that should be
    // generated after all lane edits are applied.
    TrafficSignal(traffic_signal_data::TrafficSignal),
    Closed,
}

/// This must contain all crossing turns at one intersection, each mapped either to Crosswalk or
/// UnmarkedCrossing
#[derive(Debug, Clone, PartialEq)]
pub struct EditCrosswalks(pub BTreeMap<TurnID, TurnType>);

impl EditRoad {
    pub fn get_orig_from_osm(r: &Road, cfg: &MapConfig) -> EditRoad {
        EditRoad {
            lanes_ltr: get_lane_specs_ltr(&r.osm_tags, cfg),
            speed_limit: r.speed_limit_from_osm(),
            access_restrictions: r.access_restrictions_from_osm(),
        }
    }

    fn diff(&self, other: &EditRoad) -> Vec<String> {
        #![allow(clippy::comparison_chain)]
        let mut lt = 0;
        let mut dir = 0;
        let mut width = 0;
        for (spec1, spec2) in self.lanes_ltr.iter().zip(other.lanes_ltr.iter()) {
            if spec1.lt != spec2.lt {
                lt += 1;
            }
            if spec1.dir != spec2.dir {
                dir += 1;
            }
            if spec1.width != spec2.width {
                width += 1;
            }
        }

        let mut changes = Vec::new();
        if lt == 1 {
            changes.push("1 lane type".to_string());
        } else if lt > 1 {
            changes.push(format!("{} lane types", lt));
        }
        if dir == 1 {
            changes.push("1 lane reversal".to_string());
        } else if dir > 1 {
            changes.push(format!("{} lane reversals", dir));
        }
        if width == 1 {
            changes.push("1 lane width".to_string());
        } else {
            changes.push(format!("{} lane widths", width));
        }
        if self.speed_limit != other.speed_limit {
            changes.push("speed limit".to_string());
        }
        if self.access_restrictions != other.access_restrictions {
            changes.push("access restrictions".to_string());
        }
        changes
    }
}

impl MapEdits {
    pub(crate) fn new() -> MapEdits {
        MapEdits {
            edits_name: "TODO temporary".to_string(),
            proposal_description: Vec::new(),
            proposal_link: None,
            commands: Vec::new(),
            merge_zones: true,

            changed_roads: BTreeSet::new(),
            original_intersections: BTreeMap::new(),
            original_crosswalks: BTreeMap::new(),
            changed_routes: BTreeSet::new(),
        }
    }

    /// Load map edits from a JSON file. Strip out any commands that're broken because they don't
    /// match the current map. If the resulting edits are totally empty, consider that a failure --
    /// the edits likely don't cover this map at all.
    pub fn load_from_file(map: &Map, path: String, timer: &mut Timer) -> Result<MapEdits> {
        let perma = match abstio::maybe_read_json::<PermanentMapEdits>(path.clone(), timer) {
            Ok(perma) => perma,
            Err(_) => {
                // The JSON format may have changed, so attempt backwards compatibility.
                let bytes = abstio::slurp_file(path)?;
                let value = serde_json::from_slice(&bytes)?;
                compat::upgrade(value, map)?
            }
        };

        // Don't compare the full MapName; edits in one part of a city could apply to another. But
        // make sure at least the city matches. Otherwise, we spend time trying to match up edits,
        // and produce noisy logs along the way.
        if map.get_name().city != perma.map_name.city {
            bail!(
                "Edits are for {:?}, but this map is {:?}",
                perma.map_name.city,
                map.get_name().city
            );
        }

        let edits = perma.into_edits_permissive(map);
        if edits.commands.is_empty() {
            bail!("None of the edits apply to this map");
        }
        Ok(edits)
    }

    /// Load map edits from the given JSON bytes. Strip out any commands that're broken because
    /// they don't match the current map. If the resulting edits are totally empty, consider that a
    /// failure -- the edits likely don't cover this map at all.
    pub fn load_from_bytes(map: &Map, bytes: Vec<u8>) -> Result<MapEdits> {
        let perma = match abstutil::from_json::<PermanentMapEdits>(&bytes) {
            Ok(perma) => perma,
            Err(_) => {
                // The JSON format may have changed, so attempt backwards compatibility.
                let contents = std::str::from_utf8(&bytes)?;
                let value = serde_json::from_str(contents)?;
                compat::upgrade(value, map)?
            }
        };
        let edits = perma.into_edits_permissive(map);
        if edits.commands.is_empty() {
            bail!("None of the edits apply to this map");
        }
        Ok(edits)
    }

    fn save(&self, map: &Map) {
        // If untitled and empty, don't actually save anything.
        if self.edits_name.starts_with("Untitled Proposal") && self.commands.is_empty() {
            return;
        }

        abstio::write_json(
            abstio::path_edits(map.get_name(), &self.edits_name),
            &self.to_permanent(map),
        );
    }

    fn update_derived(&mut self, map: &Map) {
        self.changed_roads.clear();
        self.original_intersections.clear();
        self.original_crosswalks.clear();
        self.changed_routes.clear();

        for cmd in &self.commands {
            match cmd {
                EditCmd::ChangeRoad { r, .. } => {
                    self.changed_roads.insert(*r);
                }
                EditCmd::ChangeIntersection { i, ref old, .. } => {
                    if !self.original_intersections.contains_key(i) {
                        self.original_intersections.insert(*i, old.clone());
                    }
                }
                EditCmd::ChangeCrosswalks { i, ref old, .. } => {
                    if !self.original_crosswalks.contains_key(i) {
                        self.original_crosswalks.insert(*i, old.clone());
                    }
                }
                EditCmd::ChangeRouteSchedule { id, .. } => {
                    self.changed_routes.insert(*id);
                }
            }
        }

        self.changed_roads.retain(|r| {
            map.get_r_edit(*r) != EditRoad::get_orig_from_osm(map.get_r(*r), &map.config)
        });
        self.original_intersections
            .retain(|i, orig| map.get_i_edit(*i) != orig.clone());
        self.original_crosswalks
            .retain(|i, orig| map.get_i_crosswalks_edit(*i) != orig.clone());
        self.changed_routes.retain(|br| {
            let r = map.get_tr(*br);
            r.spawn_times != r.orig_spawn_times
        });
    }

    /// Assumes update_derived has been called.
    pub fn compress(&mut self, map: &Map) {
        for r in &self.changed_roads {
            self.commands.push(EditCmd::ChangeRoad {
                r: *r,
                old: EditRoad::get_orig_from_osm(map.get_r(*r), &map.config),
                new: map.get_r_edit(*r),
            });
        }
        for (i, old) in &self.original_intersections {
            self.commands.push(EditCmd::ChangeIntersection {
                i: *i,
                old: old.clone(),
                new: map.get_i_edit(*i),
            });
        }
        for (i, old) in &self.original_crosswalks {
            self.commands.push(EditCmd::ChangeCrosswalks {
                i: *i,
                old: old.clone(),
                new: map.get_i_crosswalks_edit(*i),
            });
        }
        for r in &self.changed_routes {
            let r = map.get_tr(*r);
            self.commands.push(EditCmd::ChangeRouteSchedule {
                id: r.id,
                new: r.spawn_times.clone(),
                old: r.orig_spawn_times.clone(),
            });
        }
    }

    /// Pick apart changed_roads and figure out if an entire road was edited, or just a few lanes.
    /// Doesn't return deleted lanes.
    pub fn changed_lanes(&self, map: &Map) -> (BTreeSet<LaneID>, BTreeSet<RoadID>) {
        let mut lanes = BTreeSet::new();
        let mut roads = BTreeSet::new();
        for r in &self.changed_roads {
            let r = map.get_r(*r);
            let orig = EditRoad::get_orig_from_osm(r, map.get_config());
            // What exactly changed?
            if r.speed_limit != orig.speed_limit
                || r.access_restrictions != orig.access_restrictions
                // If a lane was added or deleted, figuring out if any were modified is kind of
                // unclear -- just mark the entire road.
                || r.lanes.len() != orig.lanes_ltr.len()
            {
                roads.insert(r.id);
            } else {
                for (l, spec) in r.lanes.iter().zip(orig.lanes_ltr.iter()) {
                    if l.dir != spec.dir || l.lane_type != spec.lt || l.width != spec.width {
                        lanes.insert(l.id);
                    }
                }
            }
        }
        (lanes, roads)
    }

    /// Produces an md5sum of the contents of the edits.
    pub fn get_checksum(&self, map: &Map) -> String {
        let bytes = abstutil::to_json(&self.to_permanent(map));
        let mut context = md5::Context::new();
        context.consume(&bytes);
        format!("{:x}", context.compute())
    }

    /// Get the human-friendly of these edits. If they have a description, the first line is the
    /// title. Otherwise we use the filename.
    pub fn get_title(&self) -> &str {
        if self.proposal_description.is_empty() {
            &self.edits_name
        } else {
            &self.proposal_description[0]
        }
    }
}

impl Default for MapEdits {
    fn default() -> MapEdits {
        MapEdits::new()
    }
}

impl EditCmd {
    /// (summary, details)
    pub fn describe(&self, map: &Map) -> (String, Vec<String>) {
        let mut details = Vec::new();
        let summary = match self {
            EditCmd::ChangeRoad { r, old, new } => {
                details = new.diff(old);
                format!("road #{}", r.0)
            }
            // TODO Describe changes
            EditCmd::ChangeIntersection { i, new, .. } => match new {
                EditIntersection::StopSign(_) => format!("stop sign #{}", i.0),
                EditIntersection::TrafficSignal(_) => format!("traffic signal #{}", i.0),
                EditIntersection::Closed => format!("close {}", i),
            },
            EditCmd::ChangeCrosswalks { i, .. } => format!("crosswalks at {}", i),
            EditCmd::ChangeRouteSchedule { id, .. } => {
                format!("reschedule route {}", map.get_tr(*id).short_name)
            }
        };
        (summary, details)
    }
}

impl Map {
    pub fn new_edits(&self) -> MapEdits {
        let mut edits = MapEdits::new();

        // Automatically find a new filename
        let mut i = 1;
        loop {
            let name = format!("Untitled Proposal {}", i);
            if !abstio::file_exists(abstio::path_edits(&self.name, &name)) {
                edits.edits_name = name;
                return edits;
            }
            i += 1;
        }
    }

    pub fn get_edits(&self) -> &MapEdits {
        &self.edits
    }

    pub fn unsaved_edits(&self) -> bool {
        self.edits.edits_name.starts_with("Untitled Proposal") && !self.edits.commands.is_empty()
    }

    pub fn get_r_edit(&self, r: RoadID) -> EditRoad {
        let r = self.get_r(r);
        EditRoad {
            lanes_ltr: r.lane_specs(),
            speed_limit: r.speed_limit,
            access_restrictions: r.access_restrictions.clone(),
        }
    }

    pub fn edit_road_cmd<F: Fn(&mut EditRoad)>(&self, r: RoadID, f: F) -> EditCmd {
        let old = self.get_r_edit(r);
        let mut new = old.clone();
        f(&mut new);
        EditCmd::ChangeRoad { r, old, new }
    }

    pub fn get_i_edit(&self, i: IntersectionID) -> EditIntersection {
        match self.get_i(i).control {
            IntersectionControl::Signed | IntersectionControl::Uncontrolled => {
                EditIntersection::StopSign(self.get_stop_sign(i).clone())
            }
            IntersectionControl::Signalled => {
                EditIntersection::TrafficSignal(self.get_traffic_signal(i).export(self))
            }
            IntersectionControl::Construction => EditIntersection::Closed,
        }
    }

    pub fn get_i_crosswalks_edit(&self, i: IntersectionID) -> EditCrosswalks {
        let mut turns = BTreeMap::new();
        for turn in &self.get_i(i).turns {
            if turn.turn_type.pedestrian_crossing() {
                turns.insert(turn.id, turn.turn_type);
            }
        }
        EditCrosswalks(turns)
    }

    pub fn save_edits(&self) {
        // Don't overwrite the current edits with the compressed first. Otherwise, undo/redo order
        // in the UI gets messed up.
        let mut edits = self.edits.clone();
        edits.commands.clear();
        edits.compress(self);
        edits.save(self);
    }

    /// Since the player is in the middle of editing, the signal may not be valid. Don't go through
    /// the entire apply_edits flow.
    pub fn incremental_edit_traffic_signal(&mut self, signal: ControlTrafficSignal) {
        assert_eq!(
            self.get_i(signal.id).control,
            IntersectionControl::Signalled
        );
        self.traffic_signals.insert(signal.id, signal);
    }

    /// If you need to regenerate anything when the map is edited, use this key to detect edits.
    pub fn get_edits_change_key(&self) -> usize {
        self.edits_generation
    }
}
