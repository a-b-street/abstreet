use std::collections::{BTreeMap, BTreeSet, HashSet};

use abstutil::Timer;
use geom::{Distance, HashablePt2D, Line};
use osm2streets::{osm, InputRoad};

use crate::make::{match_points_to_lanes, snap_driveway, trim_path};
use crate::{
    connectivity, BuildingID, ControlStopSign, ControlTrafficSignal, EditCmd, EditEffects,
    EditIntersectionControl, IntersectionControl, IntersectionID, LaneSpec, Map, MapEdits,
    Movement, ParkingLotID, PathConstraints, Pathfinder, RoadID, Zone,
};

impl Map {
    /// Returns (changed_roads, deleted_lanes, deleted_turns, added_turns, changed_intersections)
    pub fn must_apply_edits(&mut self, new_edits: MapEdits, timer: &mut Timer) -> EditEffects {
        self.apply_edits(new_edits, true, timer)
    }

    pub fn try_apply_edits(&mut self, new_edits: MapEdits, timer: &mut Timer) {
        self.apply_edits(new_edits, false, timer);
    }

    /// Whatever edits have been applied, treat as the basemap. This erases all commands and
    /// knowledge of what roads/intersections/etc looked like before.
    pub fn treat_edits_as_basemap(&mut self) {
        self.edits = self.new_edits();
    }

    // new_edits don't necessarily have to be valid; this could be used for speculatively testing
    // edits. Doesn't update pathfinding yet.
    fn apply_edits(
        &mut self,
        mut new_edits: MapEdits,
        enforce_valid: bool,
        timer: &mut Timer,
    ) -> EditEffects {
        self.edits_generation += 1;

        let mut effects = EditEffects {
            changed_roads: BTreeSet::new(),
            deleted_lanes: BTreeSet::new(),
            changed_intersections: BTreeSet::new(),
            added_turns: BTreeSet::new(),
            deleted_turns: BTreeSet::new(),
            changed_parking_lots: BTreeSet::new(),
            modified_lanes: BTreeSet::new(),
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

        timer.start_iter("undo old edits", self.edits.commands.len() - start_at_idx);
        for _ in start_at_idx..self.edits.commands.len() {
            timer.next();
            self.edits
                .commands
                .pop()
                .unwrap()
                .undo()
                .apply(&mut effects, self);
        }

        timer.start_iter("apply new edits", new_edits.commands.len() - start_at_idx);
        for cmd in &new_edits.commands[start_at_idx..] {
            timer.next();
            cmd.apply(&mut effects, self);
        }

        timer.start("re-snap buildings");
        let mut recalc_buildings = Vec::new();
        for b in self.all_buildings() {
            if effects.modified_lanes.contains(&b.sidewalk()) {
                recalc_buildings.push(b.id);
            }
        }
        fix_building_driveways(self, recalc_buildings, &mut effects);
        timer.stop("re-snap buildings");

        timer.start("re-snap parking lots");
        let mut recalc_parking_lots = Vec::new();
        for pl in self.all_parking_lots() {
            if effects.modified_lanes.contains(&pl.driving_pos.lane())
                || effects.modified_lanes.contains(&pl.sidewalk_pos.lane())
            {
                recalc_parking_lots.push(pl.id);
                effects.changed_parking_lots.insert(pl.id);
            }
        }
        fix_parking_lot_driveways(self, recalc_parking_lots);
        timer.stop("re-snap parking lots");

        // Might need to update bus stops.
        if enforce_valid {
            for id in &effects.changed_roads {
                let stops = self.get_r(*id).transit_stops.clone();
                for s in stops {
                    let sidewalk_pos = self.get_ts(s).sidewalk_pos;
                    // Must exist, because we aren't allowed to orphan a bus stop.
                    let driving_lane = self
                        .get_r(*id)
                        .find_closest_lane(sidewalk_pos.lane(), |l| {
                            PathConstraints::Bus.can_use(l, self)
                        })
                        .unwrap();
                    let driving_pos = sidewalk_pos.equiv_pos(driving_lane, self);
                    self.transit_stops.get_mut(&s).unwrap().driving_pos = driving_pos;
                }
            }
        }

        new_edits.update_derived(self);
        self.edits = new_edits;
        self.pathfinder_dirty = true;

        if !effects.changed_roads.is_empty() {
            self.zones = Zone::make_all(self);
        }

        // Some of these might've been added, then later deleted.
        effects
            .added_turns
            .retain(|t| self.maybe_get_t(*t).is_some());

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

        self.recalculate_road_to_buildings();

        effects
    }

    /// This can expensive, so don't constantly do it while editing in the UI. But this must happen
    /// before the simulation resumes.
    pub fn recalculate_pathfinding_after_edits(&mut self, timer: &mut Timer) {
        if !self.pathfinder_dirty {
            return;
        }

        let mut pathfinder = std::mem::replace(&mut self.pathfinder, Pathfinder::empty());
        pathfinder.apply_edits(self, timer);
        self.pathfinder = pathfinder;

        // Also recompute blackholes. This is cheap enough to do from scratch.
        timer.start("recompute blackholes");
        for road in &mut self.roads {
            for lane in &mut road.lanes {
                lane.driving_blackhole = false;
                lane.biking_blackhole = false;
            }
        }
        for l in connectivity::find_scc(self, PathConstraints::Car).1 {
            self.mut_lane(l).driving_blackhole = true;
        }
        for l in connectivity::find_scc(self, PathConstraints::Bike).1 {
            self.mut_lane(l).biking_blackhole = true;
        }
        timer.stop("recompute blackholes");

        self.pathfinder_dirty = false;
    }
}

impl EditCmd {
    // Must be idempotent
    fn apply(&self, effects: &mut EditEffects, map: &mut Map) {
        match self {
            EditCmd::ChangeRoad { r, ref new, .. } => {
                let old_state = map.get_r_edit(*r);
                if old_state == new.clone() {
                    return;
                }

                if old_state.lanes_ltr != new.lanes_ltr {
                    modify_lanes(map, *r, new.lanes_ltr.clone(), effects);
                }
                let road = &mut map.roads[r.0];
                road.speed_limit = new.speed_limit;
                road.access_restrictions = new.access_restrictions.clone();
                road.modal_filter = new.modal_filter.clone();
                road.crossings = new.crossings.clone();
                road.turn_restrictions = new.turn_restrictions.clone();
                road.complicated_turn_restrictions = new.complicated_turn_restrictions.clone();

                effects.changed_roads.insert(road.id);
                // TODO If lanes_ltr didn't change, can we skip some of this?
                for i in [road.src_i, road.dst_i] {
                    effects.changed_intersections.insert(i);
                    let i = &mut map.intersections[i.0];
                    i.outgoing_lanes.clear();
                    i.incoming_lanes.clear();
                    for r in &i.roads {
                        for lane in &map.roads[r.0].lanes {
                            if lane.src_i == i.id {
                                i.outgoing_lanes.push(lane.id);
                            } else {
                                assert_eq!(lane.dst_i, i.id);
                                i.incoming_lanes.push(lane.id);
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
                map.intersections[i.0].modal_filter = new.modal_filter.clone();

                map.stop_signs.remove(i);
                map.traffic_signals.remove(i);
                effects.changed_intersections.insert(*i);
                match new.control {
                    EditIntersectionControl::StopSign(ref ss) => {
                        map.intersections[i.0].control = IntersectionControl::Signed;
                        map.stop_signs.insert(*i, ss.clone());
                    }
                    EditIntersectionControl::TrafficSignal(ref raw_ts) => {
                        map.intersections[i.0].control = IntersectionControl::Signalled;
                        if old.control == EditIntersectionControl::Closed {
                            recalculate_turns(*i, map, effects);
                        }
                        map.traffic_signals.insert(
                            *i,
                            ControlTrafficSignal::import(raw_ts.clone(), *i, map).unwrap(),
                        );
                    }
                    EditIntersectionControl::Closed => {
                        map.intersections[i.0].control = IntersectionControl::Construction;
                    }
                }

                if old.control == EditIntersectionControl::Closed
                    || new.control == EditIntersectionControl::Closed
                {
                    recalculate_turns(*i, map, effects);
                }

                for (turn, turn_type) in &new.crosswalks {
                    map.mut_turn(*turn).turn_type = *turn_type;
                }
            }
            EditCmd::ChangeRouteSchedule { id, new, .. } => {
                map.transit_routes[id.0].spawn_times = new.clone();
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
    for t in std::mem::take(&mut i.turns) {
        effects.deleted_turns.insert(t.id);
        old_turns.push(t);
    }

    if i.is_closed() {
        return;
    }

    {
        let turns = crate::make::turns::make_all_turns(map, map.get_i(id));
        let i = &mut map.intersections[id.0];
        for t in turns {
            effects.added_turns.insert(t.id);
            i.turns.push(t);
        }
    }
    let movements = Movement::for_i(id, map);
    let i = &mut map.intersections[id.0];
    i.movements = movements;

    match i.control {
        IntersectionControl::Signed | IntersectionControl::Uncontrolled => {
            // Stop sign policy usually doesn't depend on incoming lane types, except when changing
            // to/from construction. To be safe, always regenerate. Edits to stop signs are rare
            // anyway. And when we're smarter about preserving traffic signal changes in the face
            // of lane changes, we can do the same here.
            map.stop_signs.insert(id, ControlStopSign::new(map, id));
        }
        IntersectionControl::Signalled => {
            map.traffic_signals
                .insert(id, ControlTrafficSignal::new(map, id));
        }
        IntersectionControl::Construction => unreachable!(),
    }
}

fn modify_lanes(map: &mut Map, r: RoadID, lanes_ltr: Vec<LaneSpec>, effects: &mut EditEffects) {
    // First update intersection geometry and re-trim the road centers.
    let mut road_geom_changed = Vec::new();
    {
        let road = map.get_r(r);
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

    {
        let road = &mut map.roads[r.0];
        // TODO Revisit -- effects.deleted_lanes should probably totally go away. It is used below,
        // though
        for lane in &road.lanes {
            effects.deleted_lanes.insert(lane.id);
        }
        road.recreate_lanes(lanes_ltr);
    }

    // We might've affected the geometry of other nearby roads.
    for r in road_geom_changed {
        effects.changed_roads.insert(r);
        let lane_specs = map.get_r(r).lane_specs();
        let road = &mut map.roads[r.0];
        road.recreate_lanes(lane_specs);
        for lane in &road.lanes {
            effects.modified_lanes.insert(lane.id);
        }
    }
    effects.modified_lanes.extend(effects.deleted_lanes.clone());
}

// Returns the other roads affected by this change, not counting changed_road.
fn recalculate_intersection_polygon(
    map: &mut Map,
    changed_road: RoadID,
    changed_road_width: Distance,
    i: IntersectionID,
) -> Vec<RoadID> {
    let intersection = map.get_i(i);

    let mut input_roads = Vec::new();
    for r in &intersection.roads {
        let r = map.get_r(*r);

        let total_width = if r.id == changed_road {
            changed_road_width
        } else {
            r.get_width()
        };

        input_roads.push(InputRoad {
            // Just map our IDs to something in osm2streets ID space.
            id: osm2streets::RoadID(r.id.0),
            src_i: osm2streets::IntersectionID(r.src_i.0),
            dst_i: osm2streets::IntersectionID(r.dst_i.0),
            center_line: r.untrimmed_center_pts.clone(),
            total_width,
            highway_type: r.osm_tags.get(osm::HIGHWAY).unwrap().to_string(),
        });
    }

    let results = match osm2streets::intersection_polygon(
        osm2streets::IntersectionID(intersection.id.0),
        input_roads,
        // For consolidated intersections, it appears we don't need to pass in
        // trim_roads_for_merging. May revisit this later if needed.
        &BTreeMap::new(),
    ) {
        Ok(results) => results,
        Err(err) => {
            error!("Couldn't recalculate {i}'s geometry: {err}");
            return Vec::new();
        }
    };

    map.intersections[i.0].polygon = results.intersection_polygon;

    // Recalculate trimmed centers
    let mut affected = Vec::new();
    for (id, dist) in results.trim_starts {
        let id = RoadID(id.0);
        let road = &mut map.roads[id.0];
        road.trim_start = dist;
        if let Some(pl) = osm2streets::Road::trim_polyline_both_ends(
            road.untrimmed_center_pts.clone(),
            road.trim_start,
            road.trim_end,
        ) {
            road.center_pts = pl;
        } else {
            // If the road geometrically vanishes, don't do anything for now
            error!("{} on trim_start broke", road.id);
        }
        if id != changed_road {
            affected.push(id);
        }
    }
    for (id, dist) in results.trim_ends {
        let id = RoadID(id.0);
        let road = &mut map.roads[id.0];
        road.trim_end = dist;
        if let Some(pl) = osm2streets::Road::trim_polyline_both_ends(
            road.untrimmed_center_pts.clone(),
            road.trim_start,
            road.trim_end,
        ) {
            road.center_pts = pl;
        } else {
            error!("{} on trim_end broke", road.id);
        }
        if id != changed_road {
            affected.push(id);
        }
    }
    affected
}

/// Recalculate the driveways of some buildings after map edits.
fn fix_building_driveways(map: &mut Map, input: Vec<BuildingID>, effects: &mut EditEffects) {
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
        map,
        query,
        |l| l.is_walkable(),
        // Don't put connections too close to intersections
        sidewalk_buffer,
        // Try not to skip any buildings, but more than 1km from a sidewalk is a little much
        Distance::meters(1000.0),
        &mut Timer::throwaway(),
    );

    for (id, bldg_center) in center_per_bldg {
        match sidewalk_pts.remove(&bldg_center).and_then(|pos| {
            Line::new(bldg_center.to_pt2d(), pos.pt(map))
                .map(|l| (pos, trim_path(&map.get_b(id).polygon, l)))
                .ok()
        }) {
            Some((sidewalk_pos, driveway_geom)) => {
                let b = &mut map.buildings[id.0];
                b.sidewalk_pos = sidewalk_pos;
                b.driveway_geom = driveway_geom.to_polyline();
                // We may need to redraw the road that now has this building snapped to it
                effects.changed_roads.insert(sidewalk_pos.lane().road);
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
        map,
        query,
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
