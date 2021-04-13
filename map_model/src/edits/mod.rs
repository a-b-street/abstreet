//! Once a Map exists, the player can edit it in the UI (producing `MapEdits` in-memory), then save
//! the changes to a file (as `PermanentMapEdits`). See
//! <https://a-b-street.github.io/docs/map/edits.html>.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::{retain_btreemap, retain_btreeset, Timer};
use geom::{Distance, HashablePt2D, Line, Speed, Time};

pub use self::perma::PermanentMapEdits;
use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::make::{match_points_to_lanes, snap_driveway, trim_path};
use crate::{
    connectivity, AccessRestrictions, BuildingID, BusRouteID, ControlStopSign,
    ControlTrafficSignal, IntersectionID, IntersectionType, LaneID, LaneSpec, Map, MapConfig,
    ParkingLotID, PathConstraints, Pathfinder, Road, RoadID, TurnID, Zone,
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
    pub lanes_ltr: Vec<LaneSpec>,
    pub speed_limit: Speed,
    pub access_restrictions: AccessRestrictions,
}

impl EditRoad {
    pub fn get_orig_from_osm(r: &Road, cfg: &MapConfig) -> EditRoad {
        EditRoad {
            lanes_ltr: get_lane_specs_ltr(&r.osm_tags, cfg),
            speed_limit: r.speed_limit_from_osm(),
            access_restrictions: r.access_restrictions_from_osm(),
        }
    }

    fn diff(&self, other: &EditRoad) -> Vec<String> {
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
            changes.push(format!("1 lane type"));
        } else if lt > 1 {
            changes.push(format!("{} lane types", lt));
        }
        if dir == 1 {
            changes.push(format!("1 lane reversal"));
        } else if dir > 1 {
            changes.push(format!("{} lane reversals", dir));
        }
        if width == 1 {
            changes.push(format!("1 lane width"));
        } else {
            changes.push(format!("{} lane widths", width));
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
    pub deleted_lanes: BTreeSet<LaneID>,
    pub changed_intersections: BTreeSet<IntersectionID>,
    pub added_turns: BTreeSet<TurnID>,
    pub deleted_turns: BTreeSet<TurnID>,
    pub resnapped_buildings: bool,
    pub changed_parking_lots: BTreeSet<ParkingLotID>,
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
                let bytes = abstio::slurp_file(path)?;
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
            {
                roads.insert(r.id);
            } else {
                let lanes_ltr = r.lanes_ltr();
                if lanes_ltr.len() != orig.lanes_ltr.len() {
                    // If a lane was added or deleted, figuring out if any were modified is kind of
                    // unclear -- just mark the entire road.
                    roads.insert(r.id);
                } else {
                    for ((l, dir, lt), spec) in lanes_ltr.into_iter().zip(orig.lanes_ltr.iter()) {
                        if dir != spec.dir || lt != spec.lt || map.get_l(l).width != spec.width {
                            lanes.insert(l);
                        }
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
    fn apply(&self, effects: &mut EditEffects, map: &mut Map) {
        match self {
            EditCmd::ChangeRoad { r, ref new, .. } => {
                if map.get_r_edit(*r) == new.clone() {
                    return;
                }

                modify_lanes(map, *r, new.lanes_ltr.clone(), effects);
                let road = &mut map.roads[r.0];
                road.speed_limit = new.speed_limit;
                road.access_restrictions = new.access_restrictions.clone();

                effects.changed_roads.insert(road.id);
                for i in vec![road.src_i, road.dst_i] {
                    effects.changed_intersections.insert(i);
                    let i = &mut map.intersections[i.0];
                    i.outgoing_lanes.clear();
                    i.incoming_lanes.clear();
                    for r in &i.roads {
                        for (l, _, _) in map.roads[r.0].lanes_ltr() {
                            if map.lanes[&l].src_i == i.id {
                                i.outgoing_lanes.push(l);
                            } else {
                                assert_eq!(map.lanes[&l].dst_i, i.id);
                                i.incoming_lanes.push(l);
                            }
                        }
                    }

                    recalculate_turns(i.id, map, effects);
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
                            recalculate_turns(*i, map, effects);
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
                    recalculate_turns(*i, map, effects);
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
fn recalculate_turns(id: IntersectionID, map: &mut Map, effects: &mut EditEffects) {
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

    let turns = crate::make::turns::make_all_turns(map, map.get_i(id));
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
                .insert(id, ControlTrafficSignal::new(map, id));
        }
        IntersectionType::Border | IntersectionType::Construction => unreachable!(),
    }
}

fn modify_lanes(map: &mut Map, r: RoadID, lanes_ltr: Vec<LaneSpec>, effects: &mut EditEffects) {
    let road = &mut map.roads[r.0];

    // TODO Widening roads is still experimental. If we're just modifying lane types, preserve
    // LaneIDs.
    if road.lanes_ltr.len() == lanes_ltr.len() {
        for (idx, spec) in lanes_ltr.into_iter().enumerate() {
            let lane = map.lanes.get_mut(&road.lanes_ltr[idx].0).unwrap();
            road.lanes_ltr[idx].2 = spec.lt;
            lane.lane_type = spec.lt;

            // Direction change?
            if road.lanes_ltr[idx].1 != spec.dir {
                road.lanes_ltr[idx].1 = spec.dir;
                std::mem::swap(&mut lane.src_i, &mut lane.dst_i);
                lane.lane_center_pts = lane.lane_center_pts.reversed();
                lane.dir = spec.dir;
            }

            // TODO If width is changing and the number of lanes isn't, we'll ignore the width
            // change. Don't use this old code-path for that!
        }
        return;
    }

    // First update intersection geometry and re-trim the road centers.
    let mut road_geom_changed = Vec::new();
    {
        let (src_i, dst_i) = (road.src_i, road.dst_i);
        let changed_road_width = lanes_ltr.iter().map(|spec| spec.width).sum();
        road_geom_changed.extend(recalculate_intersection_polygon(
            map,
            r,
            changed_road_width,
            src_i,
        ));
        road_geom_changed.extend(recalculate_intersection_polygon(
            map,
            r,
            changed_road_width,
            dst_i,
        ));
    }

    // Reborrow
    let road = &mut map.roads[r.0];

    // We may be adding lanes, deleting lanes, or just modifying existing ones. The width of
    // existing lanes may change. We could try to preserve existing LaneIDs and modify them, but
    // it's simpler to just delete all of the lanes and create them again.

    for (l, _, _) in road.lanes_ltr.drain(..) {
        map.lanes.remove(&l).unwrap();
        effects.deleted_lanes.insert(l);
    }

    // Create all of the road's lanes again, assigning new IDs.
    // This approach creates gaps in the lane ID space, since it deletes the contiguous block of a
    // road's lanes, then creates it again at the end. If this winds up mattering, we could use
    // different approaches for "filling in the gaps."
    let new_lanes = road.create_lanes(lanes_ltr, &mut map.lane_id_counter);
    for lane in &new_lanes {
        road.lanes_ltr.push((lane.id, lane.dir, lane.lane_type));
    }
    for lane in new_lanes {
        map.lanes.insert(lane.id, lane);
    }

    // We might've affected the geometry of other nearby roads. Recalculate the lanes for them as
    // well, but don't change the IDs.
    let mut modified_lanes = BTreeSet::new();
    for r in road_geom_changed {
        effects.changed_roads.insert(r);
        let mut dummy_id_counter = 0;
        let lanes_ltr = map.get_r(r).lane_specs(map);
        let real_lane_ids: Vec<LaneID> = map
            .get_r(r)
            .lanes_ltr()
            .into_iter()
            .map(|(l, _, _)| l)
            .collect();
        for (lane, id) in map
            .get_r(r)
            .create_lanes(lanes_ltr, &mut dummy_id_counter)
            .into_iter()
            .zip(real_lane_ids.into_iter())
        {
            map.lanes.get_mut(&id).unwrap().lane_center_pts = lane.lane_center_pts;
            modified_lanes.insert(id);
        }
    }
    modified_lanes.extend(effects.deleted_lanes.clone());

    // Find all buildings connected to modified/deleted sidewalks
    let mut recalc_buildings = Vec::new();
    for b in map.all_buildings() {
        if modified_lanes.contains(&b.sidewalk()) {
            recalc_buildings.push(b.id);
            effects.resnapped_buildings = true;
        }
    }
    fix_building_driveways(map, recalc_buildings);

    // Same for parking lots
    let mut recalc_parking_lots = Vec::new();
    for pl in map.all_parking_lots() {
        if modified_lanes.contains(&pl.driving_pos.lane())
            || modified_lanes.contains(&pl.sidewalk_pos.lane())
        {
            recalc_parking_lots.push(pl.id);
            effects.changed_parking_lots.insert(pl.id);
        }
    }
    fix_parking_lot_driveways(map, recalc_parking_lots);

    // TODO We need to update bus stops -- they may refer to an old ID.
}

// Returns the other roads affected by this change, not counting changed_road.
fn recalculate_intersection_polygon(
    map: &mut Map,
    changed_road: RoadID,
    changed_road_width: Distance,
    i: IntersectionID,
) -> Vec<RoadID> {
    use crate::make::initial;

    let intersection = map.get_i(i);

    let mut intersection_roads = BTreeSet::new();
    let mut roads = BTreeMap::new();
    let mut modify_roads = Vec::new();
    for r in &intersection.roads {
        let r = map.get_r(*r);
        modify_roads.push((r.orig_id, r.id));
        intersection_roads.insert(r.orig_id);
        let half_width = if r.id == changed_road {
            changed_road_width / 2.0
        } else {
            r.get_half_width(map)
        };
        roads.insert(
            r.orig_id,
            initial::Road {
                id: r.orig_id,
                src_i: r.orig_id.i1,
                dst_i: r.orig_id.i2,
                // TODO Untrim?
                trimmed_center_pts: r.center_pts.clone(),
                half_width,
                // Unused
                lane_specs_ltr: Vec::new(),
                osm_tags: r.osm_tags.clone(),
            },
        );
    }

    let polygon =
        initial::intersection_polygon(intersection.orig_id, intersection_roads.clone(), &mut roads)
            .unwrap()
            .0;

    map.intersections[i.0].polygon = polygon;
    // Copy over the re-trimmed road centers
    let mut affected = Vec::new();
    for (orig_id, id) in modify_roads {
        map.roads[id.0].center_pts = roads.remove(&orig_id).unwrap().trimmed_center_pts;
        if id != changed_road {
            affected.push(id);
        }
    }
    affected
}

/// Recalculate the driveways of some buildings after map edits.
fn fix_building_driveways(map: &mut Map, input: Vec<BuildingID>) {
    // TODO Copying from make/buildings.rs
    let mut center_per_bldg: BTreeMap<BuildingID, HashablePt2D> = BTreeMap::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    for id in input {
        let center = map.get_b(id).polygon.center().to_hashable();
        center_per_bldg.insert(id, center);
        query.insert(center);
    }

    let sidewalk_buffer = Distance::meters(7.5);
    let mut sidewalk_pts = match_points_to_lanes(
        map.get_bounds(),
        query,
        map.all_lanes(),
        |l| l.is_walkable(),
        // Don't put connections too close to intersections
        sidewalk_buffer,
        // Try not to skip any buildings, but more than 1km from a sidewalk is a little much
        Distance::meters(1000.0),
        &mut Timer::throwaway(),
    );

    for (id, bldg_center) in center_per_bldg {
        match sidewalk_pts.remove(&bldg_center).and_then(|pos| {
            match Line::new(bldg_center.to_pt2d(), pos.pt(map)) {
                Some(l) => Some((pos, trim_path(&map.get_b(id).polygon, l))),
                None => None,
            }
        }) {
            Some((sidewalk_pos, driveway_geom)) => {
                let b = &mut map.buildings[id.0];
                b.sidewalk_pos = sidewalk_pos;
                b.driveway_geom = driveway_geom.to_polyline();
            }
            None => {
                // TODO Not sure what to do here yet.
                error!("{} isn't snapped to a sidewalk now!", id);
            }
        }
    }
}

/// Recalculate the driveways of some parking lots after map edits.
fn fix_parking_lot_driveways(map: &mut Map, input: Vec<ParkingLotID>) {
    // TODO Partly copying from make/parking_lots.rs
    let mut center_per_lot: Vec<(ParkingLotID, HashablePt2D)> = Vec::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    for id in input {
        let center = map.get_pl(id).polygon.center().to_hashable();
        center_per_lot.push((id, center));
        query.insert(center);
    }

    let sidewalk_buffer = Distance::meters(7.5);
    let sidewalk_pts = match_points_to_lanes(
        map.get_bounds(),
        query,
        map.all_lanes(),
        |l| l.is_walkable(),
        sidewalk_buffer,
        Distance::meters(1000.0),
        &mut Timer::throwaway(),
    );

    for (id, center) in center_per_lot {
        match snap_driveway(center, &map.get_pl(id).polygon, &sidewalk_pts, map) {
            Ok((driveway_line, driving_pos, sidewalk_line, sidewalk_pos)) => {
                let pl = &mut map.parking_lots[id.0];
                pl.driveway_line = driveway_line;
                pl.driving_pos = driving_pos;
                pl.sidewalk_line = sidewalk_line;
                pl.sidewalk_pos = sidewalk_pos;
            }
            Err(err) => {
                // TODO Not sure what to do here yet.
                error!("{} isn't snapped to a sidewalk now: {}", id, err);
            }
        }
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
            lanes_ltr: r.lane_specs(self),
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

    /// Returns (changed_roads, deleted_lanes, deleted_turns, added_turns, changed_intersections)
    pub fn must_apply_edits(&mut self, new_edits: MapEdits) -> EditEffects {
        self.apply_edits(new_edits, true)
    }

    pub fn try_apply_edits(&mut self, new_edits: MapEdits) {
        self.apply_edits(new_edits, false);
    }

    // new_edits don't necessarily have to be valid; this could be used for speculatively testing
    // edits. Doesn't update pathfinding yet.
    fn apply_edits(&mut self, mut new_edits: MapEdits, enforce_valid: bool) -> EditEffects {
        let mut effects = EditEffects {
            changed_roads: BTreeSet::new(),
            deleted_lanes: BTreeSet::new(),
            changed_intersections: BTreeSet::new(),
            added_turns: BTreeSet::new(),
            deleted_turns: BTreeSet::new(),
            resnapped_buildings: false,
            changed_parking_lots: BTreeSet::new(),
        };

        // Short-circuit to avoid marking pathfinder_dirty
        if self.edits == new_edits {
            return effects;
        }

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
                .apply(&mut effects, self);
        }

        // Apply new edits.
        for cmd in &new_edits.commands[start_at_idx..] {
            cmd.apply(&mut effects, self);
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

        // Some of these might've been added, then later deleted.
        retain_btreeset(&mut effects.added_turns, |t| self.turns.contains_key(t));

        let mut more_changed_intersections = Vec::new();
        for t in effects
            .deleted_turns
            .iter()
            .chain(effects.added_turns.iter())
        {
            more_changed_intersections.push(t.parent);
        }
        effects
            .changed_intersections
            .extend(more_changed_intersections);

        effects
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
        for l in self.lanes.values_mut() {
            l.driving_blackhole = false;
            l.biking_blackhole = false;
        }
        for l in connectivity::find_scc(self, PathConstraints::Car).1 {
            self.lanes.get_mut(&l).unwrap().driving_blackhole = true;
        }
        for l in connectivity::find_scc(self, PathConstraints::Bike).1 {
            self.lanes.get_mut(&l).unwrap().biking_blackhole = true;
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
