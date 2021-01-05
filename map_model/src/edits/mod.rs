//! Once a Map exists, the player can edit it in the UI (producing `MapEdits` in-memory), then save
//! the changes to a file (as `PermanentMapEdits`). See
//! <https://dabreegster.github.io/abstreet/map/edits.html>.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::{retain_btreemap, retain_btreeset, Timer};
use geom::{Speed, Time};

pub use self::perma::PermanentMapEdits;
use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::{
    connectivity, AccessRestrictions, BusRouteID, ControlStopSign, ControlTrafficSignal, Direction,
    IntersectionID, IntersectionType, LaneID, LaneType, Map, MapConfig, PathConstraints,
    Pathfinder, Road, RoadID, TurnID, Zone,
};

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
    pub changed_routes: BTreeSet<BusRouteID>,

    /// Some edits are included in the game by default, in data/system/proposals, as "community
    /// proposals." They require a description and may have a link to a write-up.
    pub proposal_description: Vec<String>,
    pub proposal_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditIntersection {
    StopSign(ControlStopSign),
    // Don't keep ControlTrafficSignal here, because it contains movements that should be
    // generated after all lane edits are applied.
    TrafficSignal(traffic_signal_data::TrafficSignal),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditRoad {
    pub lanes_ltr: Vec<(LaneType, Direction)>,
    pub speed_limit: Speed,
    pub access_restrictions: AccessRestrictions,
}

impl EditRoad {
    pub fn get_orig_from_osm(r: &Road, cfg: &MapConfig) -> EditRoad {
        EditRoad {
            lanes_ltr: get_lane_specs_ltr(&r.osm_tags, cfg)
                .into_iter()
                .map(|spec| (spec.lt, spec.dir))
                .collect(),
            speed_limit: r.speed_limit_from_osm(),
            access_restrictions: r.access_restrictions_from_osm(),
        }
    }

    fn diff(&self, other: &EditRoad) -> Vec<String> {
        let mut lt = 0;
        let mut dir = 0;
        for ((lt1, dir1), (lt2, dir2)) in self.lanes_ltr.iter().zip(other.lanes_ltr.iter()) {
            if lt1 != lt2 {
                lt += 1;
            }
            if dir1 != dir2 {
                dir += 1;
            }
        }

        let mut changes = Vec::new();
        if lt == 1 {
            changes.push(format!("1 lane type"));
        } else if lt > 1 {
            changes.push(format!("{} lane types", lt));
        }
        if dir == 1 {
            changes.push(format!("1 lane reversal"));
        } else if dir > 1 {
            changes.push(format!("{} lane reversal", dir));
        }
        if self.speed_limit != other.speed_limit {
            changes.push(format!("speed limit"));
        }
        if self.access_restrictions != other.access_restrictions {
            changes.push(format!("access restrictions"));
        }
        changes
    }
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
        id: BusRouteID,
        old: Vec<Time>,
        new: Vec<Time>,
    },
}

pub struct EditEffects {
    pub changed_roads: BTreeSet<RoadID>,
    pub changed_intersections: BTreeSet<IntersectionID>,
    pub added_turns: BTreeSet<TurnID>,
    pub deleted_turns: BTreeSet<TurnID>,
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
            changed_routes: BTreeSet::new(),
        }
    }

    pub fn load(map: &Map, path: String, timer: &mut Timer) -> Result<MapEdits> {
        match abstio::maybe_read_json::<PermanentMapEdits>(path.clone(), timer) {
            Ok(perma) => perma.to_edits(map),
            Err(_) => {
                // The JSON format may have changed, so attempt backwards compatibility.
                let bytes = abstio::slurp_file(&path)?;
                let contents = std::str::from_utf8(&bytes)?;
                let value = serde_json::from_str(contents)?;
                let perma = compat::upgrade(value, map)?;
                perma.to_edits(map)
            }
        }
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
                EditCmd::ChangeRouteSchedule { id, .. } => {
                    self.changed_routes.insert(*id);
                }
            }
        }

        retain_btreeset(&mut self.changed_roads, |r| {
            map.get_r_edit(*r) != EditRoad::get_orig_from_osm(map.get_r(*r), &map.config)
        });
        retain_btreemap(&mut self.original_intersections, |i, orig| {
            map.get_i_edit(*i) != orig.clone()
        });
        retain_btreeset(&mut self.changed_routes, |br| {
            let r = map.get_br(*br);
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
        for r in &self.changed_routes {
            let r = map.get_br(*r);
            self.commands.push(EditCmd::ChangeRouteSchedule {
                id: r.id,
                new: r.spawn_times.clone(),
                old: r.orig_spawn_times.clone(),
            });
        }
    }

    /// Pick apart changed_roads and figure out if an entire road was edited, or just a few lanes.
    pub fn changed_lanes(&self, map: &Map) -> (BTreeSet<LaneID>, BTreeSet<RoadID>) {
        let mut lanes = BTreeSet::new();
        let mut roads = BTreeSet::new();
        for r in &self.changed_roads {
            let r = map.get_r(*r);
            let orig = EditRoad::get_orig_from_osm(r, map.get_config());
            // What exactly changed?
            if r.speed_limit != orig.speed_limit
                || r.access_restrictions != orig.access_restrictions
            {
                roads.insert(r.id);
            } else {
                let lanes_ltr = r.lanes_ltr();
                for (idx, (lt, dir)) in orig.lanes_ltr.into_iter().enumerate() {
                    if lanes_ltr[idx].1 != dir || lanes_ltr[idx].2 != lt {
                        lanes.insert(lanes_ltr[idx].0);
                    }
                }
            }
        }
        (lanes, roads)
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
            EditCmd::ChangeRouteSchedule { id, .. } => {
                format!("reschedule route {}", map.get_br(*id).short_name)
            }
        };
        (summary, details)
    }

    // Must be idempotent
    fn apply(&self, effects: &mut EditEffects, map: &mut Map, timer: &mut Timer) {
        match self {
            EditCmd::ChangeRoad { r, ref new, .. } => {
                if map.get_r_edit(*r) == new.clone() {
                    return;
                }

                let road = &mut map.roads[r.0];
                road.speed_limit = new.speed_limit;
                road.access_restrictions = new.access_restrictions.clone();
                assert_eq!(road.lanes_ltr.len(), new.lanes_ltr.len());
                for (idx, (lt, dir)) in new.lanes_ltr.clone().into_iter().enumerate() {
                    let lane = &mut map.lanes[(road.lanes_ltr[idx].0).0];
                    road.lanes_ltr[idx].2 = lt;
                    lane.lane_type = lt;

                    // Direction change?
                    if road.lanes_ltr[idx].1 != dir {
                        road.lanes_ltr[idx].1 = dir;
                        std::mem::swap(&mut lane.src_i, &mut lane.dst_i);
                        lane.lane_center_pts = lane.lane_center_pts.reversed();
                    }
                }

                effects.changed_roads.insert(road.id);
                for i in vec![road.src_i, road.dst_i] {
                    effects.changed_intersections.insert(i);
                    let i = &mut map.intersections[i.0];
                    i.outgoing_lanes.clear();
                    i.incoming_lanes.clear();
                    for r in &i.roads {
                        for (l, _, _) in map.roads[r.0].lanes_ltr() {
                            if map.lanes[l.0].src_i == i.id {
                                i.outgoing_lanes.push(l);
                            } else {
                                assert_eq!(map.lanes[l.0].dst_i, i.id);
                                i.incoming_lanes.push(l);
                            }
                        }
                    }

                    recalculate_turns(i.id, map, effects, timer);
                }
            }
            EditCmd::ChangeIntersection {
                i,
                ref new,
                ref old,
            } => {
                if map.get_i_edit(*i) == new.clone() {
                    return;
                }

                map.stop_signs.remove(i);
                map.traffic_signals.remove(i);
                effects.changed_intersections.insert(*i);
                match new {
                    EditIntersection::StopSign(ref ss) => {
                        map.intersections[i.0].intersection_type = IntersectionType::StopSign;
                        map.stop_signs.insert(*i, ss.clone());
                    }
                    EditIntersection::TrafficSignal(ref raw_ts) => {
                        map.intersections[i.0].intersection_type = IntersectionType::TrafficSignal;
                        if old == &EditIntersection::Closed {
                            recalculate_turns(*i, map, effects, timer);
                        }
                        map.traffic_signals.insert(
                            *i,
                            ControlTrafficSignal::import(raw_ts.clone(), *i, map).unwrap(),
                        );
                    }
                    EditIntersection::Closed => {
                        map.intersections[i.0].intersection_type = IntersectionType::Construction;
                    }
                }

                if old == &EditIntersection::Closed || new == &EditIntersection::Closed {
                    recalculate_turns(*i, map, effects, timer);
                }
            }
            EditCmd::ChangeRouteSchedule { id, new, .. } => {
                map.bus_routes[id.0].spawn_times = new.clone();
            }
        }
    }

    fn undo(self) -> EditCmd {
        match self {
            EditCmd::ChangeRoad { r, old, new } => EditCmd::ChangeRoad {
                r,
                old: new,
                new: old,
            },
            EditCmd::ChangeIntersection { i, old, new } => EditCmd::ChangeIntersection {
                i,
                old: new,
                new: old,
            },
            EditCmd::ChangeRouteSchedule { id, old, new } => EditCmd::ChangeRouteSchedule {
                id,
                old: new,
                new: old,
            },
        }
    }
}

// This clobbers previously set traffic signal overrides.
// TODO Step 1: Detect and warn about that
// TODO Step 2: Avoid when possible
fn recalculate_turns(
    id: IntersectionID,
    map: &mut Map,
    effects: &mut EditEffects,
    timer: &mut Timer,
) {
    let i = &mut map.intersections[id.0];

    if i.is_border() {
        assert!(i.turns.is_empty());
        return;
    }

    let mut old_turns = Vec::new();
    for t in std::mem::replace(&mut i.turns, BTreeSet::new()) {
        old_turns.push(map.turns.remove(&t).unwrap());
        effects.deleted_turns.insert(t);
    }

    if i.is_closed() {
        return;
    }

    let turns = crate::make::turns::make_all_turns(map, map.get_i(id), timer);
    let i = &mut map.intersections[id.0];
    for t in turns {
        effects.added_turns.insert(t.id);
        i.turns.insert(t.id);
        if let Some(_existing_t) = old_turns.iter().find(|turn| turn.id == t.id) {
            // TODO Except for lookup_idx
            //assert_eq!(t, *existing_t);
        }
        map.turns.insert(t.id, t);
    }

    match i.intersection_type {
        IntersectionType::StopSign => {
            // Stop sign policy usually doesn't depend on incoming lane types, except when changing
            // to/from construction. To be safe, always regenerate. Edits to stop signs are rare
            // anyway. And when we're smarter about preserving traffic signal changes in the face
            // of lane changes, we can do the same here.
            map.stop_signs.insert(id, ControlStopSign::new(map, id));
        }
        IntersectionType::TrafficSignal => {
            map.traffic_signals
                .insert(id, ControlTrafficSignal::new(map, id, timer));
        }
        IntersectionType::Border | IntersectionType::Construction => unreachable!(),
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
            lanes_ltr: r
                .lanes_ltr()
                .into_iter()
                .map(|(_, dir, lt)| (lt, dir))
                .collect(),
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

    /// Panics on borders
    pub fn get_i_edit(&self, i: IntersectionID) -> EditIntersection {
        match self.get_i(i).intersection_type {
            IntersectionType::StopSign => EditIntersection::StopSign(self.get_stop_sign(i).clone()),
            IntersectionType::TrafficSignal => {
                EditIntersection::TrafficSignal(self.get_traffic_signal(i).export(self))
            }
            IntersectionType::Construction => EditIntersection::Closed,
            IntersectionType::Border => unreachable!(),
        }
    }

    pub fn save_edits(&self) {
        // Don't overwrite the current edits with the compressed first. Otherwise, undo/redo order
        // in the UI gets messed up.
        let mut edits = self.edits.clone();
        edits.commands.clear();
        edits.compress(self);
        edits.save(self);
    }

    pub fn must_apply_edits(
        &mut self,
        new_edits: MapEdits,
        timer: &mut Timer,
    ) -> (
        BTreeSet<RoadID>,
        BTreeSet<TurnID>,
        BTreeSet<TurnID>,
        BTreeSet<IntersectionID>,
    ) {
        self.apply_edits(new_edits, true, timer)
    }

    pub fn try_apply_edits(&mut self, new_edits: MapEdits, timer: &mut Timer) {
        self.apply_edits(new_edits, false, timer);
    }

    // new_edits don't necessarily have to be valid; this could be used for speculatively testing
    // edits. Returns roads changed, turns deleted, turns added, intersections modified. Doesn't
    // update pathfinding yet.
    fn apply_edits(
        &mut self,
        mut new_edits: MapEdits,
        enforce_valid: bool,
        timer: &mut Timer,
    ) -> (
        BTreeSet<RoadID>,
        BTreeSet<TurnID>,
        BTreeSet<TurnID>,
        BTreeSet<IntersectionID>,
    ) {
        // Short-circuit to avoid marking pathfinder_dirty
        if self.edits == new_edits {
            return (
                BTreeSet::new(),
                BTreeSet::new(),
                BTreeSet::new(),
                BTreeSet::new(),
            );
        }

        let mut effects = EditEffects::new();

        // We need to undo() all of the current commands in reverse order, then apply() all of the
        // new commands. But in many cases, new_edits is just the current edits with a few commands
        // at the end. So a simple optimization with equivalent behavior is to skip the common
        // prefix of commands.
        let mut start_at_idx = 0;
        for (cmd1, cmd2) in self.edits.commands.iter().zip(new_edits.commands.iter()) {
            if cmd1 == cmd2 {
                start_at_idx += 1;
            } else {
                break;
            }
        }

        // Undo existing edits
        for _ in start_at_idx..self.edits.commands.len() {
            self.edits
                .commands
                .pop()
                .unwrap()
                .undo()
                .apply(&mut effects, self, timer);
        }

        // Apply new edits.
        for cmd in &new_edits.commands[start_at_idx..] {
            cmd.apply(&mut effects, self, timer);
        }

        // Might need to update bus stops.
        if enforce_valid {
            for id in &effects.changed_roads {
                let stops = self.get_r(*id).all_bus_stops(self);
                for s in stops {
                    let sidewalk_pos = self.get_bs(s).sidewalk_pos;
                    // Must exist, because we aren't allowed to orphan a bus stop.
                    let driving_lane = self
                        .get_r(*id)
                        .find_closest_lane(
                            sidewalk_pos.lane(),
                            |l| PathConstraints::Bus.can_use(l, self),
                            self,
                        )
                        .unwrap();
                    let driving_pos = sidewalk_pos.equiv_pos(driving_lane, self);
                    self.bus_stops.get_mut(&s).unwrap().driving_pos = driving_pos;
                }
            }
        }

        let merge_zones_changed = self.edits.merge_zones != new_edits.merge_zones;

        new_edits.update_derived(self);
        self.edits = new_edits;
        self.pathfinder_dirty = true;

        // Update zones after setting the new edits, since it'll pull merge_zones from there
        if !effects.changed_roads.is_empty() || merge_zones_changed {
            self.zones = Zone::make_all(self);
        }

        (
            effects.changed_roads,
            effects.deleted_turns,
            // Some of these might've been added, then later deleted.
            effects
                .added_turns
                .into_iter()
                .filter(|t| self.turns.contains_key(t))
                .collect(),
            effects.changed_intersections,
        )
    }

    /// This can expensive, so don't constantly do it while editing in the UI. But this must happen
    /// before the simulation resumes.
    pub fn recalculate_pathfinding_after_edits(&mut self, timer: &mut Timer) {
        if !self.pathfinder_dirty {
            return;
        }

        let mut pathfinder = std::mem::replace(&mut self.pathfinder, Pathfinder::Dijkstra);
        pathfinder.apply_edits(self, timer);
        self.pathfinder = pathfinder;

        // Also recompute blackholes. This is cheap enough to do from scratch.
        timer.start("recompute blackholes");
        for l in self.lanes.iter_mut() {
            l.driving_blackhole = false;
            l.biking_blackhole = false;
        }
        for l in connectivity::find_scc(self, PathConstraints::Car).1 {
            self.lanes[l.0].driving_blackhole = true;
        }
        for l in connectivity::find_scc(self, PathConstraints::Bike).1 {
            self.lanes[l.0].biking_blackhole = true;
        }
        timer.stop("recompute blackholes");

        self.pathfinder_dirty = false;
    }

    /// Since the player is in the middle of editing, the signal may not be valid. Don't go through
    /// the entire apply_edits flow.
    pub fn incremental_edit_traffic_signal(&mut self, signal: ControlTrafficSignal) {
        assert_eq!(
            self.get_i(signal.id).intersection_type,
            IntersectionType::TrafficSignal
        );
        self.traffic_signals.insert(signal.id, signal);
    }
}
