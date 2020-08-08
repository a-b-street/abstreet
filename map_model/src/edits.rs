use crate::raw::{OriginalIntersection, OriginalRoad};
use crate::{
    connectivity, osm, BusRouteID, ControlStopSign, ControlTrafficSignal, IntersectionID,
    IntersectionType, LaneID, LaneType, Map, PathConstraints, RoadID, TurnID, Zone,
};
use abstutil::{deserialize_btreemap, retain_btreemap, retain_btreeset, serialize_btreemap, Timer};
use enumset::EnumSet;
use geom::{Speed, Time};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct MapEdits {
    pub edits_name: String,
    pub commands: Vec<EditCmd>,

    // Derived from commands, kept up to date by update_derived
    pub original_lts: BTreeMap<LaneID, LaneType>,
    pub reversed_lanes: BTreeSet<LaneID>,
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

#[derive(Debug, Clone, PartialEq)]
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
    ChangeAccessRestrictions {
        id: RoadID,
        // All means it's not a zone
        new_allow_through_traffic: EnumSet<PathConstraints>,
        old_allow_through_traffic: EnumSet<PathConstraints>,
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
            original_intersections: BTreeMap::new(),
            changed_speed_limits: BTreeSet::new(),
            changed_access_restrictions: BTreeSet::new(),
            changed_routes: BTreeSet::new(),
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
    fn save(&self, map: &Map) {
        assert_ne!(self.edits_name, "untitled edits");

        abstutil::write_json(
            abstutil::path_edits(map.get_name(), &self.edits_name),
            &PermanentMapEdits::to_permanent(self, map),
        );
    }

    fn update_derived(&mut self, map: &Map) {
        let mut orig_lts = BTreeMap::new();
        let mut reversed_lanes = BTreeSet::new();
        let mut orig_intersections: BTreeMap<IntersectionID, EditIntersection> = BTreeMap::new();
        let mut changed_speed_limits = BTreeSet::new();
        let mut changed_access_restrictions = BTreeSet::new();
        let mut changed_routes = BTreeSet::new();

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
                EditCmd::ChangeAccessRestrictions { id, .. } => {
                    changed_access_restrictions.insert(*id);
                }
                EditCmd::ChangeRouteSchedule { id, .. } => {
                    changed_routes.insert(*id);
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
        retain_btreeset(&mut changed_access_restrictions, |r| {
            let r = map.get_r(*r);
            r.access_restrictions_from_osm() != r.allow_through_traffic
        });
        retain_btreeset(&mut changed_routes, |br| {
            let r = map.get_br(*br);
            r.spawn_times != r.orig_spawn_times
        });

        self.original_lts = orig_lts;
        self.reversed_lanes = reversed_lanes;
        self.original_intersections = orig_intersections;
        self.changed_speed_limits = changed_speed_limits;
        self.changed_access_restrictions = changed_access_restrictions;
        self.changed_routes = changed_routes;
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
                new_allow_through_traffic: map.get_r(*r).allow_through_traffic,
                old_allow_through_traffic: map.get_r(*r).access_restrictions_from_osm(),
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

// These mirror the above, except they use permanent IDs that have a better chance of surviving
// basemap updates over time.

#[derive(Serialize, Deserialize, Clone)]
pub struct PermanentMapEdits {
    pub map_name: String,
    pub edits_name: String,
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

// Enough data to notice when lanes along a road have changed
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OriginalLane {
    pub parent: OriginalRoad,
    pub num_fwd: usize,
    pub num_back: usize,
    pub fwd: bool,
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
    ChangeAccessRestrictions {
        id: OriginalRoad,
        new_allow_through_traffic: EnumSet<PathConstraints>,
        old_allow_through_traffic: EnumSet<PathConstraints>,
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
                    EditCmd::ChangeIntersection { i, new, old } => {
                        PermanentEditCmd::ChangeIntersection {
                            i: map.get_i(*i).orig_id,
                            new: new.to_permanent(map),
                            old: old.to_permanent(map),
                        }
                    }
                    EditCmd::ChangeAccessRestrictions {
                        id,
                        new_allow_through_traffic,
                        old_allow_through_traffic,
                    } => PermanentEditCmd::ChangeAccessRestrictions {
                        id: map.get_r(*id).orig_id,
                        new_allow_through_traffic: *new_allow_through_traffic,
                        old_allow_through_traffic: *old_allow_through_traffic,
                    },
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
                    PermanentEditCmd::ChangeAccessRestrictions {
                        id,
                        new_allow_through_traffic,
                        old_allow_through_traffic,
                    } => {
                        let id = map.find_r_by_osm_id(id)?;
                        Ok(EditCmd::ChangeAccessRestrictions {
                            id,
                            new_allow_through_traffic,
                            old_allow_through_traffic,
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

            original_lts: BTreeMap::new(),
            reversed_lanes: BTreeSet::new(),
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
        let (fwd, idx) = r.dir_and_offset(l);
        OriginalLane {
            parent: r.orig_id,
            num_fwd: r.children_forwards.len(),
            num_back: r.children_backwards.len(),
            fwd,
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
        if r.children_forwards.len() != self.num_fwd || r.children_backwards.len() != self.num_back
        {
            return Err(format!("number of lanes has changed in {:?}", self));
        }
        Ok(r.children(self.fwd)[self.idx].0)
    }
}

impl EditCmd {
    pub fn short_name(&self, map: &Map) -> String {
        match self {
            EditCmd::ChangeLaneType { lt, id, .. } => format!("{} on #{}", lt.short_name(), id.0),
            EditCmd::ReverseLane { l, .. } => format!("reverse {}", l),
            EditCmd::ChangeSpeedLimit { id, new, .. } => format!("limit {} for {}", new, id),
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
                let (fwds, idx) = r.dir_and_offset(id);
                r.children_mut(fwds)[idx] = (id, lt);

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
                let dir = *dst_i == r.dst_i;
                assert_eq!(r.children_mut(!dir).remove(0).0, l);
                r.children_mut(dir).insert(0, (l, lane.lane_type));
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
            EditCmd::ChangeAccessRestrictions {
                id,
                new_allow_through_traffic,
                ..
            } => {
                if map.get_r(*id).allow_through_traffic == *new_allow_through_traffic {
                    return false;
                }
                map.roads[id.0].allow_through_traffic = *new_allow_through_traffic;
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
            EditCmd::ChangeAccessRestrictions {
                id,
                old_allow_through_traffic,
                new_allow_through_traffic,
            } => EditCmd::ChangeAccessRestrictions {
                id: *id,
                old_allow_through_traffic: *new_allow_through_traffic,
                new_allow_through_traffic: *old_allow_through_traffic,
            }
            .apply(effects, map, timer),
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

        let mut pathfinder = self.pathfinder.take().unwrap();
        pathfinder.apply_edits(self, timer);
        self.pathfinder = Some(pathfinder);

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
