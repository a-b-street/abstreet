//! Once a Map exists, the player can edit it in the UI (producing `MapEdits` in-memory), then save
//! the changes to a file (as `PermanentMapEdits`). See
//! <https://a-b-street.github.io/docs/tech/map/edits.html>.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::{Speed, Time};
use osm2streets::{get_lane_specs_ltr, RestrictionType};

pub use self::perma::PermanentMapEdits;
use crate::{
    AccessRestrictions, BuildingID, ControlStopSign, ControlTrafficSignal, Crossing,
    DiagonalFilter, IntersectionControl, IntersectionID, LaneID, LaneSpec, Map, MapConfig,
    OffstreetParking, ParkingLotID, Road, RoadFilter, RoadID, TransitRouteID, TurnID, TurnType,
};

mod apply;
mod compat;
mod perma;
pub mod perma_traffic_signal;

/// Represents changes to a map. Note this isn't serializable -- that's what `PermanentMapEdits`
/// does.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEdits {
    pub edits_name: String,
    /// A stack, oldest edit is first. The same object may be edited multiple times in this stack,
    /// until compress() happens.
    pub commands: Vec<EditCmd>,

    /// Derived from commands, kept up to date by update_derived
    pub original_roads: BTreeMap<RoadID, EditRoad>,
    pub original_intersections: BTreeMap<IntersectionID, EditIntersection>,
    pub changed_routes: BTreeSet<TransitRouteID>,
    pub original_buildings: BTreeMap<BuildingID, EditBuilding>,

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
    ChangeBuilding {
        b: BuildingID,
        old: EditBuilding,
        new: EditBuilding,
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
    pub modal_filter: Option<RoadFilter>,
    pub crossings: Vec<Crossing>,
    pub turn_restrictions: Vec<(RestrictionType, RoadID)>,
    pub complicated_turn_restrictions: Vec<(RoadID, RoadID)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditIntersection {
    pub control: EditIntersectionControl,
    pub modal_filter: Option<DiagonalFilter>,
    /// This must contain all crossing turns at one intersection, each mapped either to Crosswalk
    /// or UnmarkedCrossing
    pub crosswalks: BTreeMap<TurnID, TurnType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditBuilding {
    pub parking: OffstreetParking,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditIntersectionControl {
    StopSign(ControlStopSign),
    // Don't keep ControlTrafficSignal here, because it contains movements that should be
    // generated after all lane edits are applied.
    TrafficSignal(perma_traffic_signal::TrafficSignal),
    Closed,
}

impl EditRoad {
    pub fn get_orig_from_osm(r: &Road, cfg: &MapConfig) -> EditRoad {
        EditRoad {
            lanes_ltr: get_lane_specs_ltr(&r.osm_tags, cfg),
            speed_limit: r.speed_limit_from_osm(),
            access_restrictions: r.access_restrictions_from_osm(),
            // TODO Port logic/existing_filters.rs here?
            modal_filter: None,
            // TODO From crossing_nodes?
            crossings: Vec::new(),
            // TODO - review this. When editing turn restrictions, within the LTN tool we do not
            // use `get_orig_from_osm()`. The `EditRoad` is populated `map.get_r_edit()`.
            // Therefore we just create empty vecs here for now.
            // See https://github.com/a-b-street/abstreet/pull/1091#discussion_r1311717165
            turn_restrictions: Vec::new(),
            complicated_turn_restrictions: Vec::new(),
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
        if self.modal_filter != other.modal_filter {
            changes.push("modal filter".to_string());
        }
        if self.crossings != other.crossings {
            changes.push("crossings".to_string());
        }
        changes
    }
}

impl EditIntersection {
    fn diff(&self, other: &EditIntersection) -> Vec<String> {
        let mut changes = Vec::new();
        // TODO Could get more specific about changes to stop signs, traffic signals, etc
        if self.control != other.control {
            changes.push("control type".to_string());
        }
        if self.crosswalks != other.crosswalks {
            changes.push("crosswalks".to_string());
        }
        if self.modal_filter != other.modal_filter {
            changes.push("modal filter".to_string());
        }
        changes
    }
}

impl EditBuilding {
    fn diff(&self, other: &EditBuilding) -> Vec<String> {
        let mut changes = Vec::new();
        if self.parking != other.parking {
            changes.push("parking".to_string());
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

            original_roads: BTreeMap::new(),
            original_intersections: BTreeMap::new(),
            changed_routes: BTreeSet::new(),
            original_buildings: BTreeMap::new(),
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
        self.original_roads.clear();
        self.original_intersections.clear();
        self.changed_routes.clear();
        self.original_buildings.clear();

        for cmd in &self.commands {
            match cmd {
                EditCmd::ChangeRoad { r, ref old, .. } => {
                    if !self.original_roads.contains_key(r) {
                        self.original_roads.insert(*r, old.clone());
                    }
                }
                EditCmd::ChangeIntersection { i, ref old, .. } => {
                    if !self.original_intersections.contains_key(i) {
                        self.original_intersections.insert(*i, old.clone());
                    }
                }
                EditCmd::ChangeRouteSchedule { id, .. } => {
                    self.changed_routes.insert(*id);
                }
                EditCmd::ChangeBuilding { b, old, new } => {
                    if !self.original_buildings.contains_key(b) {
                        self.original_buildings.insert(*b, old.clone());
                    }
                }
            }
        }

        self.original_roads
            .retain(|r, orig| map.get_r_edit(*r) != orig.clone());
        self.original_intersections
            .retain(|i, orig| map.get_i_edit(*i) != orig.clone());
        self.changed_routes.retain(|br| {
            let r = map.get_tr(*br);
            r.spawn_times != r.orig_spawn_times
        });
        self.original_buildings
            .retain(|b, orig| map.get_b_edit(*b) != orig.clone());
    }

    /// Assumes update_derived has been called.
    pub fn compress(&mut self, map: &Map) {
        for (r, old) in &self.original_roads {
            self.commands.push(EditCmd::ChangeRoad {
                r: *r,
                old: old.clone(),
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
        for r in &self.changed_routes {
            let r = map.get_tr(*r);
            self.commands.push(EditCmd::ChangeRouteSchedule {
                id: r.id,
                new: r.spawn_times.clone(),
                old: r.orig_spawn_times.clone(),
            });
        }
        for (b, old) in &self.original_buildings {
            self.commands.push(EditCmd::ChangeBuilding {
                b: *b,
                old: old.clone(),
                new: map.get_b_edit(*b),
            });
        }
    }

    /// Pick apart changed_roads and figure out if an entire road was edited, or just a few lanes.
    /// Doesn't return deleted lanes.
    pub fn changed_lanes(&self, map: &Map) -> (BTreeSet<LaneID>, BTreeSet<RoadID>) {
        let mut lanes = BTreeSet::new();
        let mut roads = BTreeSet::new();
        for (r, orig) in &self.original_roads {
            let r = map.get_r(*r);
            // What exactly changed?
            if r.speed_limit != orig.speed_limit
                || r.access_restrictions != orig.access_restrictions
                || r.modal_filter != orig.modal_filter
                || r.crossings != orig.crossings
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

    pub fn is_crossing_modified(&self, r: RoadID, crossing: &Crossing) -> bool {
        if let Some(orig) = self.original_roads.get(&r) {
            !orig.crossings.contains(crossing)
        } else {
            // If this road isn't in original_roads at all, then nothing there has been modified
            false
        }
    }
    pub fn is_filter_modified(&self, r: RoadID, filter: &RoadFilter) -> bool {
        if let Some(orig) = self.original_roads.get(&r) {
            orig.modal_filter != Some(filter.clone())
        } else {
            false
        }
    }
    pub fn is_diagonal_filter_modified(&self, i: IntersectionID, filter: &DiagonalFilter) -> bool {
        if let Some(orig) = self.original_intersections.get(&i) {
            orig.modal_filter != Some(filter.clone())
        } else {
            false
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
            EditCmd::ChangeIntersection { i, old, new } => {
                details = new.diff(old);
                format!("intersection #{}", i.0)
            }
            EditCmd::ChangeRouteSchedule { id, .. } => {
                format!("reschedule route {}", map.get_tr(*id).short_name)
            }
            EditCmd::ChangeBuilding { b, old, new } => {
                details = new.diff(old);
                format!("building #{}", b.0)
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
            modal_filter: r.modal_filter.clone(),
            crossings: r.crossings.clone(),
            turn_restrictions: r.turn_restrictions.clone(),
            complicated_turn_restrictions: r.complicated_turn_restrictions.clone(),
        }
    }

    pub fn get_b_edit(&self, b: BuildingID) -> EditBuilding {
        let b = self.get_b(b);
        EditBuilding { parking: b.parking }
    }

    pub fn edit_road_cmd<F: FnOnce(&mut EditRoad)>(&self, r: RoadID, f: F) -> EditCmd {
        let old = self.get_r_edit(r);
        let mut new = old.clone();
        f(&mut new);
        EditCmd::ChangeRoad { r, old, new }
    }

    pub fn get_i_edit(&self, i: IntersectionID) -> EditIntersection {
        let i = self.get_i(i);
        let control = match i.control {
            IntersectionControl::Signed | IntersectionControl::Uncontrolled => {
                EditIntersectionControl::StopSign(self.get_stop_sign(i.id).clone())
            }
            IntersectionControl::Signalled => {
                EditIntersectionControl::TrafficSignal(self.get_traffic_signal(i.id).export(self))
            }
            IntersectionControl::Construction => EditIntersectionControl::Closed,
        };
        let mut crosswalks = BTreeMap::new();
        for turn in &i.turns {
            if turn.turn_type.pedestrian_crossing() {
                crosswalks.insert(turn.id, turn.turn_type);
            }
        }
        EditIntersection {
            control,
            modal_filter: i.modal_filter.clone(),
            crosswalks,
        }
    }

    pub fn edit_intersection_cmd<F: FnOnce(&mut EditIntersection)>(
        &self,
        i: IntersectionID,
        f: F,
    ) -> EditCmd {
        let old = self.get_i_edit(i);
        let mut new = old.clone();
        f(&mut new);
        EditCmd::ChangeIntersection { i, old, new }
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
