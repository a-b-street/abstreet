use crate::make::traffic_signals::{brute_force, get_possible_policies};
use crate::raw::OriginalRoad;
use crate::{
    osm, CompressedTurnGroupID, DirectedRoadID, Direction, IntersectionID, Map, TurnGroup,
    TurnGroupID, TurnID, TurnPriority, TurnType, YELLOW_DURATION,
};
use abstutil::{deserialize_btreemap, retain_btreeset, serialize_btreemap, Timer};
use geom::Duration;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlTrafficSignal {
    pub id: IntersectionID,
    pub stages: Vec<Stage>,
    pub offset: Duration,
    pub yellow_duration: Duration,
    pub control_type: TrafficControlType,

    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub turn_groups: BTreeMap<TurnGroupID, TurnGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    protected_groups: BTreeSet<TurnGroupID>,
    yield_groups: BTreeSet<TurnGroupID>,
    // TODO Not renaming this, because this is going to change radically in
    // https://github.com/dabreegster/abstreet/pull/298 anyway
    pub phase_type: PhaseType,
    pub minimum_green: Duration,
    pub maximum_green: Duration,
    pub passage_time: Duration,
    pub walk_time: Duration,
    pub crosswalk_clearance_time: Duration,
}

impl Stage {
    pub fn protected_groups_iter(&self) -> impl Iterator<Item = &TurnGroupID> {
        self.protected_groups.iter()
    }

    pub fn yield_groups_iter(&self) -> impl Iterator<Item = &TurnGroupID> {
        self.yield_groups.iter()
    }

    pub fn protected_groups_contains(&self, item: &TurnGroupID) -> bool {
        self.protected_groups.contains(item)
    }

    pub fn yield_groups_contains(&self, item: &TurnGroupID) -> bool {
        self.yield_groups.contains(item)
    }

    // Inserting a turn group in this way only makes sense
    // for normal Stages, not SuperStages. For a SuperStage we need
    // to know which phase to insert into. However, for simple applications
    // like drawing all turn groups in a stage, where control behavior
    // isn't relevant, we can still get away with inserting in this way.
    pub fn insert_protected_group(&mut self, tg: TurnGroupID) {
        self.protected_groups.insert(tg);
    }

    pub fn insert_yield_group(&mut self, tg: TurnGroupID){
        self.yield_groups.insert(tg);
    }

    pub fn remove_protected_group(&mut self, tg: &TurnGroupID) {
        self.protected_groups.remove(tg);
    }

    pub fn remove_yield_group(&mut self, tg: &TurnGroupID) {
        self.yield_groups.remove(tg);
    }

    pub fn num_protected_groups(&self) -> usize {
        self.protected_groups.len()
    }

    pub fn num_yield_groups(&self) -> usize {
        self.yield_groups.len()
    }

    pub fn no_protected_groups(&self) -> bool {
        self.num_protected_groups() == 0
    }

    pub fn no_yield_groups(&self) -> bool {
        self.num_yield_groups() == 0
    }

    pub fn is_subset(&self, other: &Stage) -> bool {
        self.protected_groups.is_subset(&other.protected_groups) &&
        self.yield_groups.is_subset(&other.yield_groups)
    }

    pub fn remove_non_crosswalks(&mut self, turn_groups_map: &BTreeMap<TurnGroupID, TurnGroup>) {
        retain_btreeset(&mut self.protected_groups, |g| {
            turn_groups_map[g].turn_type != TurnType::Crosswalk
        });
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TrafficControlType {
    Actuated,
    PreTimed,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum SignalTimerType {
    PassageTimer,
    MaxGreenTimer,
    YellowTimer,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PhaseType {
    Fixed(Duration),
    // Same as fixed, but when this stage would normally end, if there's still incoming demand,
    // repeat the stage entirely.
    // TODO This is a silly policy, but a start towards variable timers.
    Adaptive(Duration),
}

impl PhaseType {
    // TODO Maybe don't have this; force callers to acknowledge different policies
    pub fn simple_duration(&self) -> Duration {
        match self {
            PhaseType::Fixed(d) | PhaseType::Adaptive(d) => *d,
        }
    }
}

impl ControlTrafficSignal {
    pub fn new(map: &Map, id: IntersectionID, timer: &mut Timer) -> ControlTrafficSignal {
        let mut policies = ControlTrafficSignal::get_possible_policies(map, id, timer);
        if policies.len() == 1 {
            timer.warn(format!("Falling back to greedy_assignment for {}", id));
        }
        policies.remove(0).1
    }

    pub fn get_possible_policies(
        map: &Map,
        id: IntersectionID,
        timer: &mut Timer,
    ) -> Vec<(String, ControlTrafficSignal)> {
        get_possible_policies(map, id, timer)
    }
    // TODO tmp
    pub fn brute_force(map: &Map, id: IntersectionID) {
        brute_force(map, id)
    }

    pub fn validate(self) -> Result<ControlTrafficSignal, String> {
        // Does the assignment cover the correct set of groups?
        let expected_groups: BTreeSet<TurnGroupID> = self.turn_groups.keys().cloned().collect();
        let mut actual_groups: BTreeSet<TurnGroupID> = BTreeSet::new();
        for stage in &self.stages {
            actual_groups.extend(stage.protected_groups_iter());
            actual_groups.extend(stage.yield_groups_iter());
        }
        if expected_groups != actual_groups {
            return Err(format!(
                "Traffic signal assignment for {} broken. Missing {:?}, contains irrelevant {:?}",
                self.id,
                expected_groups
                    .difference(&actual_groups)
                    .cloned()
                    .collect::<Vec<_>>(),
                actual_groups
                    .difference(&expected_groups)
                    .cloned()
                    .collect::<Vec<_>>()
            ));
        }

        for stage in &self.stages {
            // Do any of the priority groups in one stage conflict?
            for g1 in stage.protected_groups_iter().map(|g| &self.turn_groups[g]) {
                for g2 in stage.protected_groups_iter().map(|g| &self.turn_groups[g]) {
                    if g1.conflicts_with(g2) {
                        return Err(format!(
                            "Traffic signal has conflicting protected groups in one \
                             stage:\n{:?}\n\n{:?}",
                            g1, g2
                        ));
                    }
                }
            }

            // Do any of the crosswalks yield?
            for g in stage.yield_groups_iter().map(|g| &self.turn_groups[g]) {
                assert!(g.turn_type != TurnType::Crosswalk);
            }
        }

        Ok(self)
    }

    // Returns true if this did anything
    pub fn convert_to_ped_scramble(&mut self) -> bool {
        let orig = self.clone();

        let mut all_walk_stage = Stage::new();
        for g in self.turn_groups.values() {
            if g.turn_type == TurnType::Crosswalk {
                all_walk_stage.edit_group(g, TurnPriority::Protected);
            }
        }

        // Remove Crosswalk groups from existing stages.
        let mut replaced = std::mem::replace(&mut self.stages, Vec::new());
        let mut has_all_walk = false;
        for stage in replaced.iter_mut() {
            if !has_all_walk && stage == &all_walk_stage {
                has_all_walk = true;
                continue;
            }

            // Crosswalks are only in protected_groups.
            stage.remove_non_crosswalks(&self.turn_groups);

            // Blindly try to promote yield groups to protected, now that crosswalks are gone.
            let mut promoted = Vec::new();
            for g in stage.yield_groups_iter() {
                if stage.could_be_protected(*g, &self.turn_groups) {
                    promoted.push(*g);
                }
            }
            for g in promoted {
                stage.insert_protected_group(g);
                stage.remove_yield_group(&g);
            }
        }
        self.stages = replaced;

        if !has_all_walk {
            self.stages.push(all_walk_stage);
        }
        self != &orig
    }

    pub fn turn_to_group(&self, turn: TurnID) -> TurnGroupID {
        if let Some(tg) = self
            .turn_groups
            .values()
            .find(|tg| tg.members.contains(&turn))
        {
            tg.id
        } else {
            panic!("{} doesn't belong to any turn groups in {}", turn, self.id)
        }
    }

    pub fn missing_turns(&self) -> BTreeSet<TurnGroupID> {
        let mut missing: BTreeSet<TurnGroupID> = self.turn_groups.keys().cloned().collect();
        for stage in &self.stages {
            for g in stage.protected_groups_iter() {
                missing.remove(g);
            }
            for g in stage.yield_groups_iter() {
                missing.remove(g);
            }
        }
        missing
    }

    pub fn compressed_id(&self, turn: TurnID) -> CompressedTurnGroupID {
        for (idx, tg) in self.turn_groups.values().enumerate() {
            if tg.members.contains(&turn) {
                return CompressedTurnGroupID {
                    i: self.id,
                    idx: u8::try_from(idx).unwrap(),
                };
            }
        }
        panic!("{} doesn't belong to any turn groups in {}", turn, self.id)
    }
}

impl Stage {
    pub fn new() -> Stage {
        Stage {
            protected_groups: BTreeSet::new(),
            yield_groups: BTreeSet::new(),
            phase_type: PhaseType::Fixed(Duration::seconds(30.0)),
            minimum_green: Duration::seconds(5.0),
            maximum_green: Duration::seconds(60.0),
            passage_time: Duration::seconds(4.0),
            // TODO: walk_time and crosswalk_clearance_time should be calculated based on crosswalk
            // length.
            walk_time: Duration::seconds(5.0),
            crosswalk_clearance_time: Duration::seconds(10.0),
        }
    }

    pub fn could_be_protected(
        &self,
        g1: TurnGroupID,
        turn_groups: &BTreeMap<TurnGroupID, TurnGroup>,
    ) -> bool {
        let group1 = &turn_groups[&g1];
        for g2 in self.protected_groups_iter() {
            if g1 == *g2 || group1.conflicts_with(&turn_groups[g2]) {
                return false;
            }
        }
        true
    }

    pub fn get_priority_of_turn(&self, t: TurnID, parent: &ControlTrafficSignal) -> TurnPriority {
        self.get_priority_of_group(parent.turn_to_group(t))
    }

    pub fn get_priority_of_group(&self, g: TurnGroupID) -> TurnPriority {
        if self.protected_groups_contains(&g) {
            TurnPriority::Protected
        } else if self.yield_groups_contains(&g) {
            TurnPriority::Yield
        } else {
            TurnPriority::Banned
        }
    }

    pub fn edit_group(&mut self, g: &TurnGroup, pri: TurnPriority) {
        let mut ids = vec![g.id];
        if g.turn_type == TurnType::Crosswalk {
            ids.push(TurnGroupID {
                from: g.id.to,
                to: g.id.from,
                parent: g.id.parent,
                crosswalk: true,
            });
        }
        for id in ids {
            self.remove_protected_group(&id);
            self.remove_yield_group(&id);
            if pri == TurnPriority::Protected {
                self.insert_protected_group(id);
            } else if pri == TurnPriority::Yield {
                self.insert_yield_group(id);
            }
        }
    }
}

impl ControlTrafficSignal {
    pub fn export(&self, map: &Map) -> seattle_traffic_signals::TrafficSignal {
        seattle_traffic_signals::TrafficSignal {
            intersection_osm_node_id: map.get_i(self.id).orig_id.0,
            phases: self
                .stages
                .iter()
                .map(|s| seattle_traffic_signals::Phase {
                    protected_turns: s
                        .protected_groups_iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    permitted_turns: s
                        .yield_groups_iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    phase_type: match s.phase_type {
                        PhaseType::Fixed(d) => {
                            seattle_traffic_signals::PhaseType::Fixed(d.inner_seconds() as usize)
                        }
                        PhaseType::Adaptive(d) => {
                            seattle_traffic_signals::PhaseType::Adaptive(d.inner_seconds() as usize)
                        }
                    },
                })
                .collect(),
            offset_seconds: self.offset.inner_seconds() as usize,
        }
    }

    pub fn import(
        raw: seattle_traffic_signals::TrafficSignal,
        id: IntersectionID,
        map: &Map,
    ) -> Result<ControlTrafficSignal, String> {
        let mut stages = Vec::new();
        for s in raw.phases {
            let num_protected = s.protected_turns.len();
            let num_permitted = s.permitted_turns.len();
            let protected_groups = s
                .protected_turns
                .into_iter()
                .filter_map(|t| import_turn_group(t, map))
                .collect::<BTreeSet<_>>();
            let yield_groups = s
                .permitted_turns
                .into_iter()
                .filter_map(|t| import_turn_group(t, map))
                .collect::<BTreeSet<_>>();
            if protected_groups.len() == num_protected && yield_groups.len() == num_permitted {
                stages.push(Stage {
                    protected_groups,
                    yield_groups,
                    phase_type: match s.phase_type {
                        seattle_traffic_signals::PhaseType::Fixed(d) => {
                            PhaseType::Fixed(Duration::seconds(d as f64))
                        }
                        seattle_traffic_signals::PhaseType::Adaptive(d) => {
                            PhaseType::Adaptive(Duration::seconds(d as f64))
                        }
                    },
                    minimum_green: Duration::seconds(5.0),
                    maximum_green: Duration::seconds(60.0),
                    passage_time: Duration::seconds(4.0),
                    // TODO: walk_time and crosswalk_clearance_time should be calculated based on
                    // crosswalk length.
                    walk_time: Duration::seconds(5.0),
                    crosswalk_clearance_time: Duration::seconds(10.0),
                });
            } else {
                return Err(format!(
                    "Failed to import some of the turn groups for {}",
                    raw.intersection_osm_node_id
                ));
            }
        }
        ControlTrafficSignal {
            id,
            stages,
            control_type: TrafficControlType::Actuated,
            offset: Duration::seconds(raw.offset_seconds as f64),
            yellow_duration: YELLOW_DURATION,
            turn_groups: TurnGroup::for_i(id, map).unwrap(),
        }
        .validate()
    }
}

fn export_turn_group(id: &TurnGroupID, map: &Map) -> seattle_traffic_signals::Turn {
    let from = map.get_r(id.from.id).orig_id;
    let to = map.get_r(id.to.id).orig_id;

    seattle_traffic_signals::Turn {
        from: seattle_traffic_signals::DirectedRoad {
            osm_way_id: from.osm_way_id.0,
            osm_node1: from.i1.0,
            osm_node2: from.i2.0,
            is_forwards: id.from.dir == Direction::Fwd,
        },
        to: seattle_traffic_signals::DirectedRoad {
            osm_way_id: to.osm_way_id.0,
            osm_node1: to.i1.0,
            osm_node2: to.i2.0,
            is_forwards: id.to.dir == Direction::Fwd,
        },
        intersection_osm_node_id: map.get_i(id.parent).orig_id.0,
        is_crosswalk: id.crosswalk,
    }
}

fn import_turn_group(id: seattle_traffic_signals::Turn, map: &Map) -> Option<TurnGroupID> {
    Some(TurnGroupID {
        from: find_r(id.from, map)?,
        to: find_r(id.to, map)?,
        parent: map
            .find_i_by_osm_id(osm::NodeID(id.intersection_osm_node_id))
            .ok()?,
        crosswalk: id.is_crosswalk,
    })
}

fn find_r(id: seattle_traffic_signals::DirectedRoad, map: &Map) -> Option<DirectedRoadID> {
    Some(DirectedRoadID {
        id: map
            .find_r_by_osm_id(OriginalRoad::new(
                id.osm_way_id,
                (id.osm_node1, id.osm_node2),
            ))
            .ok()?,
        dir: if id.is_forwards {
            Direction::Fwd
        } else {
            Direction::Back
        },
    })
}
