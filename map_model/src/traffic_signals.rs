use crate::make::traffic_signals::{brute_force, get_possible_policies};
use crate::{
    DirectedRoadID, IntersectionID, Map, TurnGroup, TurnGroupID, TurnID, TurnPriority, TurnType,
};
use abstutil::{deserialize_btreemap, retain_btreeset, serialize_btreemap, Timer};
use geom::Duration;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlTrafficSignal {
    pub id: IntersectionID,
    pub phases: Vec<Phase>,
    pub offset: Duration,

    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub turn_groups: BTreeMap<TurnGroupID, TurnGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Phase {
    pub protected_groups: BTreeSet<TurnGroupID>,
    pub yield_groups: BTreeSet<TurnGroupID>,
    pub phase_type: PhaseType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PhaseType {
    Fixed(Duration),
    // Same as fixed, but when this phase would normally end, if there's still incoming demand,
    // repeat the phase entirely.
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
        for phase in &self.phases {
            actual_groups.extend(phase.protected_groups.iter());
            actual_groups.extend(phase.yield_groups.iter());
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

        for phase in &self.phases {
            // Do any of the priority groups in one phase conflict?
            for g1 in phase.protected_groups.iter().map(|g| &self.turn_groups[g]) {
                for g2 in phase.protected_groups.iter().map(|g| &self.turn_groups[g]) {
                    if g1.conflicts_with(g2) {
                        return Err(format!(
                            "Traffic signal has conflicting protected groups in one \
                             phase:\n{:?}\n\n{:?}",
                            g1, g2
                        ));
                    }
                }
            }

            // Do any of the crosswalks yield?
            for g in phase.yield_groups.iter().map(|g| &self.turn_groups[g]) {
                assert!(g.turn_type != TurnType::Crosswalk);
            }
        }

        Ok(self)
    }

    // Returns true if this did anything
    pub fn convert_to_ped_scramble(&mut self) -> bool {
        let orig = self.clone();

        let mut all_walk_phase = Phase::new();
        for g in self.turn_groups.values() {
            if g.turn_type == TurnType::Crosswalk {
                all_walk_phase.edit_group(g, TurnPriority::Protected);
            }
        }

        // Remove Crosswalk groups from existing phases.
        let mut replaced = std::mem::replace(&mut self.phases, Vec::new());
        let mut has_all_walk = false;
        for phase in replaced.iter_mut() {
            if !has_all_walk && phase == &all_walk_phase {
                has_all_walk = true;
                continue;
            }

            // Crosswalks are only in protected_groups.
            retain_btreeset(&mut phase.protected_groups, |g| {
                self.turn_groups[g].turn_type != TurnType::Crosswalk
            });

            // Blindly try to promote yield groups to protected, now that crosswalks are gone.
            let mut promoted = Vec::new();
            for g in &phase.yield_groups {
                if phase.could_be_protected(*g, &self.turn_groups) {
                    phase.protected_groups.insert(*g);
                    promoted.push(*g);
                }
            }
            for g in promoted {
                phase.yield_groups.remove(&g);
            }
        }
        self.phases = replaced;

        if !has_all_walk {
            self.phases.push(all_walk_phase);
        }
        self != &orig
    }
}

impl Phase {
    pub fn new() -> Phase {
        Phase {
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
        // TODO Cache this?
        if let Some(g) = parent
            .turn_groups
            .values()
            .find(|g| g.members.contains(&t))
            .map(|g| g.id)
        {
            self.get_priority_of_group(g)
        } else {
            panic!("{} doesn't belong to any turn groups", t);
        }
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
            intersection_osm_node_id: map.get_i(self.id).orig_id.osm_node_id,
            phases: self
                .phases
                .iter()
                .map(|p| seattle_traffic_signals::Phase {
                    protected_turns: p
                        .protected_groups
                        .iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    permitted_turns: p
                        .yield_groups
                        .iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    phase_type: match p.phase_type {
                        PhaseType::Fixed(d) => {
                            seattle_traffic_signals::PhaseType::Fixed(d.inner_seconds() as usize)
                        }
                        PhaseType::Adaptive(d) => {
                            seattle_traffic_signals::PhaseType::Adaptive(d.inner_seconds() as usize)
                        }
                    },
                })
                .collect(),
        }
    }

    pub fn import(
        raw: seattle_traffic_signals::TrafficSignal,
        id: IntersectionID,
        map: &Map,
    ) -> Option<ControlTrafficSignal> {
        let mut phases = Vec::new();
        for p in raw.phases {
            let num_protected = p.protected_turns.len();
            let num_permitted = p.permitted_turns.len();
            let protected_groups = p
                .protected_turns
                .into_iter()
                .filter_map(|t| import_turn_group(t, map))
                .collect::<BTreeSet<_>>();
            let yield_groups = p
                .permitted_turns
                .into_iter()
                .filter_map(|t| import_turn_group(t, map))
                .collect::<BTreeSet<_>>();
            if protected_groups.len() == num_protected && yield_groups.len() == num_permitted {
                phases.push(Phase {
                    protected_groups,
                    yield_groups,
                    phase_type: match p.phase_type {
                        seattle_traffic_signals::PhaseType::Fixed(d) => {
                            PhaseType::Fixed(Duration::seconds(d as f64))
                        }
                        seattle_traffic_signals::PhaseType::Adaptive(d) => {
                            PhaseType::Adaptive(Duration::seconds(d as f64))
                        }
                    },
                });
            } else {
                return None;
            }
        }
        ControlTrafficSignal {
            id,
            phases,
            offset: Duration::ZERO,
            turn_groups: TurnGroup::for_i(id, map),
        }
        .validate()
        .ok()
    }
}

fn export_turn_group(id: &TurnGroupID, map: &Map) -> seattle_traffic_signals::Turn {
    let from = map.get_r(id.from.id).orig_id;
    let to = map.get_r(id.to.id).orig_id;

    seattle_traffic_signals::Turn {
        from: seattle_traffic_signals::DirectedRoad {
            osm_way_id: from.osm_way_id,
            osm_node1: from.i1.osm_node_id,
            osm_node2: from.i2.osm_node_id,
            is_forwards: id.from.forwards,
        },
        to: seattle_traffic_signals::DirectedRoad {
            osm_way_id: to.osm_way_id,
            osm_node1: to.i1.osm_node_id,
            osm_node2: to.i2.osm_node_id,
            is_forwards: id.to.forwards,
        },
        intersection_osm_node_id: map.get_i(id.parent).orig_id.osm_node_id,
        is_crosswalk: id.crosswalk,
    }
}

fn import_turn_group(id: seattle_traffic_signals::Turn, map: &Map) -> Option<TurnGroupID> {
    Some(TurnGroupID {
        from: find_r(id.from, map)?,
        to: find_r(id.to, map)?,
        parent: map.find_i_by_osm_id(id.intersection_osm_node_id).ok()?,
        crosswalk: id.is_crosswalk,
    })
}

fn find_r(id: seattle_traffic_signals::DirectedRoad, map: &Map) -> Option<DirectedRoadID> {
    Some(DirectedRoadID {
        id: map
            .find_r_by_osm_id(id.osm_way_id, (id.osm_node1, id.osm_node2))
            .ok()?,
        forwards: id.is_forwards,
    })
}
