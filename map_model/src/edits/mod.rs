mod compat;
mod perma;

use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::{
    connectivity, AccessRestrictions, BusRouteID, ControlStopSign, ControlTrafficSignal, Direction,
    IntersectionID, IntersectionType, LaneID, LaneType, Map, PathConstraints, Pathfinder, Road,
    RoadID, TurnID, Zone,
};
use abstutil::{retain_btreemap, retain_btreeset, Timer};
use geom::{Speed, Time};
pub use perma::{OriginalLane, PermanentMapEdits};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct MapEdits {
    pub edits_name: String,
    pub commands: Vec<EditCmd>,

    // Derived from commands, kept up to date by update_derived
    pub original_lts: BTreeMap<LaneID, LaneType>,
    pub reversed_lanes: BTreeSet<LaneID>,
    // TODO Do we need to store the original? We can just do EditRoad::get_orig_from_osm.
    pub original_roads: BTreeMap<RoadID, EditRoad>,
    pub original_intersections: BTreeMap<IntersectionID, EditIntersection>,
    pub changed_speed_limits: BTreeSet<RoadID>,
    pub changed_access_restrictions: BTreeSet<RoadID>,
    pub changed_routes: BTreeSet<BusRouteID>,

    // Edits without these are player generated.
    pub proposal_description: Vec<String>,
    // The link is optional even for proposals
    pub proposal_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditIntersection {
    StopSign(ControlStopSign),
    // Don't keep ControlTrafficSignal here, because it contains turn groups that should be
    // generated after all lane edits are applied.
    TrafficSignal(seattle_traffic_signals::TrafficSignal),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditRoad {
    lanes_ltr: Vec<(LaneType, Direction)>,
    speed_limit: Speed,
    access_restrictions: AccessRestrictions,
}

impl EditRoad {
    fn get_orig_from_osm(r: &Road) -> EditRoad {
        EditRoad {
            lanes_ltr: get_lane_specs_ltr(&r.osm_tags)
                .into_iter()
                .map(|spec| (spec.lt, spec.dir))
                .collect(),
            speed_limit: r.speed_limit_from_osm(),
            access_restrictions: r.access_restrictions_from_osm(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditCmd {
    ChangeRoad {
        r: RoadID,
        old: EditRoad,
        new: EditRoad,
    },
    // TODO Deprecated
    ChangeLaneType {
        id: LaneID,
        lt: LaneType,
        orig_lt: LaneType,
    },
    // TODO Deprecated
    ReverseLane {
        l: LaneID,
        // New intended dst_i
        dst_i: IntersectionID,
    },
    // TODO Deprecated
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
    // TODO Deprecated
    ChangeAccessRestrictions {
        id: RoadID,
        new: AccessRestrictions,
        old: AccessRestrictions,
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
    pub fn new() -> MapEdits {
        MapEdits {
            // Something has to fill this out later
            edits_name: "untitled edits".to_string(),
            proposal_description: Vec::new(),
            proposal_link: None,
            commands: Vec::new(),

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
            original_roads: BTreeMap::new(),
            original_intersections: BTreeMap::new(),
            changed_speed_limits: BTreeSet::new(),
            changed_access_restrictions: BTreeSet::new(),
            changed_routes: BTreeSet::new(),
        }
    }

    pub fn load(map: &Map, path: String, timer: &mut Timer) -> Result<MapEdits, String> {
        match abstutil::maybe_read_json(path.clone(), timer) {
            Ok(perma) => PermanentMapEdits::from_permanent(perma, map),
            Err(_) => {
                let bytes = abstutil::slurp_file(&path).map_err(|err| err.to_string())?;
                let contents = std::str::from_utf8(&bytes).map_err(|err| err.to_string())?;
                let value = serde_json::from_str(contents).map_err(|err| err.to_string())?;
                let perma = compat::upgrade(value)?;
                PermanentMapEdits::from_permanent(perma, map)
            }
        }
    }

    // TODO Version these? Or it's unnecessary, since we have a command stack.
    fn save(&self, map: &Map) {
        assert_ne!(self.edits_name, "untitled edits");

        abstutil::write_json(
            abstutil::path_edits(map.get_name(), &self.edits_name),
            &PermanentMapEdits::to_permanent(self, map),
        );
    }

    fn update_derived(&mut self, map: &Map) {
        self.original_lts.clear();
        self.reversed_lanes.clear();
        self.original_roads.clear();
        self.original_intersections.clear();
        self.changed_speed_limits.clear();
        self.changed_access_restrictions.clear();
        self.changed_routes.clear();

        for cmd in &self.commands {
            match cmd {
                EditCmd::ChangeLaneType { id, orig_lt, .. } => {
                    if !self.original_lts.contains_key(id) {
                        self.original_lts.insert(*id, *orig_lt);
                    }
                }
                EditCmd::ReverseLane { l, .. } => {
                    if self.reversed_lanes.contains(l) {
                        self.reversed_lanes.remove(l);
                    } else {
                        self.reversed_lanes.insert(*l);
                    }
                }
                EditCmd::ChangeSpeedLimit { id, .. } => {
                    self.changed_speed_limits.insert(*id);
                }
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
                EditCmd::ChangeAccessRestrictions { id, .. } => {
                    self.changed_access_restrictions.insert(*id);
                }
                EditCmd::ChangeRouteSchedule { id, .. } => {
                    self.changed_routes.insert(*id);
                }
            }
        }

        retain_btreemap(&mut self.original_lts, |l, lt| {
            map.get_l(*l).lane_type != *lt
        });
        retain_btreemap(&mut self.original_roads, |r, orig| {
            map.get_r_edit(*r) != orig.clone()
        });
        retain_btreemap(&mut self.original_intersections, |i, orig| {
            map.get_i_edit(*i) != orig.clone()
        });
        retain_btreeset(&mut self.changed_speed_limits, |r| {
            map.get_r(*r).speed_limit != map.get_r(*r).speed_limit_from_osm()
        });
        retain_btreeset(&mut self.changed_access_restrictions, |r| {
            let r = map.get_r(*r);
            r.access_restrictions_from_osm() != r.access_restrictions
        });
        retain_btreeset(&mut self.changed_routes, |br| {
            let r = map.get_br(*br);
            r.spawn_times != r.orig_spawn_times
        });
    }

    // Assumes update_derived has been called.
    fn compress(&mut self, map: &Map) {
        for l in &self.reversed_lanes {
            self.commands.push(EditCmd::ReverseLane {
                l: *l,
                dst_i: map.get_l(*l).dst_i,
            });
        }
        for (l, orig_lt) in &self.original_lts {
            self.commands.push(EditCmd::ChangeLaneType {
                id: *l,
                lt: map.get_l(*l).lane_type,
                orig_lt: *orig_lt,
            });
        }
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
        for r in &self.changed_speed_limits {
            self.commands.push(EditCmd::ChangeSpeedLimit {
                id: *r,
                new: map.get_r(*r).speed_limit,
                old: map.get_r(*r).speed_limit_from_osm(),
            });
        }
        for r in &self.changed_access_restrictions {
            self.commands.push(EditCmd::ChangeAccessRestrictions {
                id: *r,
                new: map.get_r(*r).access_restrictions.clone(),
                old: map.get_r(*r).access_restrictions_from_osm(),
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
    pub fn short_name(&self, map: &Map) -> String {
        match self {
            EditCmd::ChangeLaneType { lt, id, .. } => format!("{} on #{}", lt.short_name(), id.0),
            EditCmd::ReverseLane { l, .. } => format!("reverse {}", l),
            EditCmd::ChangeSpeedLimit { id, new, .. } => format!("limit {} for {}", new, id),
            // TODO Way more details
            EditCmd::ChangeRoad { r, .. } => format!("road #{}", r.0),
            EditCmd::ChangeIntersection { i, new, .. } => match new {
                EditIntersection::StopSign(_) => format!("stop sign #{}", i.0),
                EditIntersection::TrafficSignal(_) => format!("traffic signal #{}", i.0),
                EditIntersection::Closed => format!("close {}", i),
            },
            // TODO "allow/ban X on Y"
            EditCmd::ChangeAccessRestrictions { id, .. } => {
                format!("access restrictions for {}", id)
            }
            EditCmd::ChangeRouteSchedule { id, .. } => {
                format!("reschedule route {}", map.get_br(*id).short_name)
            }
        }
    }

    // Must be idempotent. True if it actually did anything.
    fn apply(&self, effects: &mut EditEffects, map: &mut Map, timer: &mut Timer) -> bool {
        match self {
            EditCmd::ChangeLaneType { id, lt, .. } => {
                let id = *id;
                let lt = *lt;

                let lane = &mut map.lanes[id.0];
                if lane.lane_type == lt {
                    return false;
                }

                lane.lane_type = lt;
                let r = &mut map.roads[lane.parent.0];
                let idx = r.offset(id);
                r.lanes_ltr[idx].2 = lt;

                effects.changed_roads.insert(lane.parent);
                effects.changed_intersections.insert(lane.src_i);
                effects.changed_intersections.insert(lane.dst_i);
                let (src_i, dst_i) = (lane.src_i, lane.dst_i);
                recalculate_turns(src_i, map, effects, timer);
                recalculate_turns(dst_i, map, effects, timer);
                true
            }
            EditCmd::ReverseLane { l, dst_i } => {
                let l = *l;
                let lane = &mut map.lanes[l.0];

                if lane.dst_i == *dst_i {
                    return false;
                }

                map.intersections[lane.src_i.0]
                    .outgoing_lanes
                    .retain(|x| *x != l);
                map.intersections[lane.dst_i.0]
                    .incoming_lanes
                    .retain(|x| *x != l);

                std::mem::swap(&mut lane.src_i, &mut lane.dst_i);
                assert_eq!(lane.dst_i, *dst_i);
                lane.lane_center_pts = lane.lane_center_pts.reversed();

                map.intersections[lane.src_i.0].outgoing_lanes.push(l);
                map.intersections[lane.dst_i.0].incoming_lanes.push(l);

                // We can only reverse the lane closest to the center.
                let r = &mut map.roads[lane.parent.0];
                let dir = if *dst_i == r.dst_i {
                    Direction::Fwd
                } else {
                    Direction::Back
                };
                let idx = r.offset(l);
                assert_eq!(r.lanes_ltr[idx].1, dir.opposite());
                r.lanes_ltr[idx].1 = dir;
                effects.changed_roads.insert(r.id);
                effects.changed_intersections.insert(lane.src_i);
                effects.changed_intersections.insert(lane.dst_i);
                let (src_i, dst_i) = (lane.src_i, lane.dst_i);
                recalculate_turns(src_i, map, effects, timer);
                recalculate_turns(dst_i, map, effects, timer);
                true
            }
            EditCmd::ChangeSpeedLimit { id, new, .. } => {
                if map.roads[id.0].speed_limit != *new {
                    map.roads[id.0].speed_limit = *new;
                    effects.changed_roads.insert(*id);
                    true
                } else {
                    false
                }
            }
            EditCmd::ChangeRoad { r, ref new, .. } => {
                if map.get_r_edit(*r) == new.clone() {
                    return false;
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
                true
            }
            EditCmd::ChangeIntersection {
                i,
                ref new,
                ref old,
            } => {
                if map.get_i_edit(*i) == new.clone() {
                    return false;
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
                true
            }
            EditCmd::ChangeAccessRestrictions { id, new, .. } => {
                if map.get_r(*id).access_restrictions == new.clone() {
                    return false;
                }
                map.roads[id.0].access_restrictions = new.clone();
                effects.changed_roads.insert(*id);
                let r = map.get_r(*id);
                effects.changed_intersections.insert(r.src_i);
                effects.changed_intersections.insert(r.dst_i);
                true
            }
            EditCmd::ChangeRouteSchedule { id, new, .. } => {
                map.bus_routes[id.0].spawn_times = new.clone();
                true
            }
        }
    }

    // Must be idempotent. True if it actually did anything.
    fn undo(&self, effects: &mut EditEffects, map: &mut Map, timer: &mut Timer) -> bool {
        match self {
            EditCmd::ChangeLaneType { id, orig_lt, lt } => EditCmd::ChangeLaneType {
                id: *id,
                lt: *orig_lt,
                orig_lt: *lt,
            }
            .apply(effects, map, timer),
            EditCmd::ReverseLane { l, dst_i } => {
                let lane = map.get_l(*l);
                let other_i = if lane.src_i == *dst_i {
                    lane.dst_i
                } else {
                    lane.src_i
                };
                EditCmd::ReverseLane {
                    l: *l,
                    dst_i: other_i,
                }
                .apply(effects, map, timer)
            }
            EditCmd::ChangeSpeedLimit { id, old, .. } => {
                if map.roads[id.0].speed_limit != *old {
                    map.roads[id.0].speed_limit = *old;
                    effects.changed_roads.insert(*id);
                    true
                } else {
                    false
                }
            }
            EditCmd::ChangeRoad {
                r,
                ref old,
                ref new,
            } => EditCmd::ChangeRoad {
                r: *r,
                old: new.clone(),
                new: old.clone(),
            }
            .apply(effects, map, timer),
            EditCmd::ChangeIntersection {
                i,
                ref old,
                ref new,
            } => EditCmd::ChangeIntersection {
                i: *i,
                old: new.clone(),
                new: old.clone(),
            }
            .apply(effects, map, timer),
            EditCmd::ChangeAccessRestrictions { id, old, new } => {
                EditCmd::ChangeAccessRestrictions {
                    id: *id,
                    old: new.clone(),
                    new: old.clone(),
                }
                .apply(effects, map, timer)
            }
            EditCmd::ChangeRouteSchedule { id, old, new } => EditCmd::ChangeRouteSchedule {
                id: *id,
                old: new.clone(),
                new: old.clone(),
            }
            .apply(effects, map, timer),
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
    pub fn get_edits(&self) -> &MapEdits {
        &self.edits
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

    // Panics on borders
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
        // TODO More efficient ways to do this: given two sets of edits, produce a smaller diff.
        // Simplest strategy: Remove common prefix.
        let mut effects = EditEffects::new();

        // First undo all existing edits.
        let mut undo = std::mem::replace(&mut self.edits.commands, Vec::new());
        undo.reverse();
        let mut undid = 0;
        for cmd in &undo {
            if cmd.undo(&mut effects, self, timer) {
                undid += 1;
            }
        }
        timer.note(format!("Undid {} / {} existing edits", undid, undo.len()));

        // Apply new edits.
        let mut applied = 0;
        for cmd in &new_edits.commands {
            if cmd.apply(&mut effects, self, timer) {
                applied += 1;
            }
        }
        timer.note(format!(
            "Applied {} / {} new edits",
            applied,
            new_edits.commands.len()
        ));

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

        if !effects.changed_roads.is_empty() {
            self.zones = Zone::make_all(self);
        }

        new_edits.update_derived(self);
        self.edits = new_edits;
        self.pathfinder_dirty = true;
        (
            // TODO We just care about contraflow roads here
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

    // Since the player is in the middle of editing, the signal may not be valid. Don't go through
    // the entire apply_edits flow.
    pub fn incremental_edit_traffic_signal(&mut self, signal: ControlTrafficSignal) {
        assert_eq!(
            self.get_i(signal.id).intersection_type,
            IntersectionType::TrafficSignal
        );
        self.traffic_signals.insert(signal.id, signal);
    }
}
