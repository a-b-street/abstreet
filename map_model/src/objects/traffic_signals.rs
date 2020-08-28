use crate::make::traffic_signals::{brute_force, get_possible_policies};
use crate::raw::OriginalRoad;
use crate::{
    osm, CompressedTurnGroupID, DirectedRoadID, Direction, IntersectionID, Map, TurnGroup,
    TurnGroupID, TurnID, TurnPriority, TurnType,
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

    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub turn_groups: BTreeMap<TurnGroupID, TurnGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    pub protected_groups: BTreeSet<TurnGroupID>,
    pub yield_groups: BTreeSet<TurnGroupID>,
    // TODO Not renaming this, because this is going to change radically in
    // https://github.com/dabreegster/abstreet/pull/298 anyway
    pub phase_type: PhaseType,
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
            actual_groups.extend(stage.protected_groups.iter());
            actual_groups.extend(stage.yield_groups.iter());
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
            for g1 in stage.protected_groups.iter().map(|g| &self.turn_groups[g]) {
                for g2 in stage.protected_groups.iter().map(|g| &self.turn_groups[g]) {
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
            for g in stage.yield_groups.iter().map(|g| &self.turn_groups[g]) {
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
            retain_btreeset(&mut stage.protected_groups, |g| {
                self.turn_groups[g].turn_type != TurnType::Crosswalk
            });

            // Blindly try to promote yield groups to protected, now that crosswalks are gone.
            let mut promoted = Vec::new();
            for g in &stage.yield_groups {
                if stage.could_be_protected(*g, &self.turn_groups) {
                    stage.protected_groups.insert(*g);
                    promoted.push(*g);
                }
            }
            for g in promoted {
                stage.yield_groups.remove(&g);
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
            for g in &stage.protected_groups {
                missing.remove(g);
            }
            for g in &stage.yield_groups {
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
        }
    }

    pub fn could_be_protected(
        &self,
        g1: TurnGroupID,
        turn_groups: &BTreeMap<TurnGroupID, TurnGroup>,
    ) -> bool {
        let group1 = &turn_groups[&g1];
        for g2 in &self.protected_groups {
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
        if self.protected_groups.contains(&g) {
            TurnPriority::Protected
        } else if self.yield_groups.contains(&g) {
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
            self.protected_groups.remove(&id);
            self.yield_groups.remove(&id);
            if pri == TurnPriority::Protected {
                self.protected_groups.insert(id);
            } else if pri == TurnPriority::Yield {
                self.yield_groups.insert(id);
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
                        .protected_groups
                        .iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    permitted_turns: s
                        .yield_groups
                        .iter()
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
            offset: Duration::seconds(raw.offset_seconds as f64),
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
