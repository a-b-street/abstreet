// A traffic signal consists of a sequence of Stages that repeat in a cycle. Most Stages last for a
// fixed duration. During a single Stage, some movements are protected (can proceed with the highest
// priority), while others are permitted (have to yield before proceeding).

use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, retain_btreeset, serialize_btreemap, Timer};
use geom::{Distance, Duration, Speed};

use crate::make::traffic_signals::{brute_force, get_possible_policies};
use crate::objects::traffic_signals::PhaseType::{Adaptive, Fixed};
use crate::raw::OriginalRoad;
use crate::{
    osm, CompressedMovementID, DirectedRoadID, Direction, IntersectionID, Map, Movement,
    MovementID, TurnID, TurnPriority, TurnType,
};

// The pace to use for crosswalk pace in m/s
// https://en.wikipedia.org/wiki/Preferred_walking_speed
const CROSSWALK_PACE: Speed = Speed::const_meters_per_second(1.4);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlTrafficSignal {
    pub id: IntersectionID,
    pub stages: Vec<Stage>,
    pub offset: Duration,

    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub movements: BTreeMap<MovementID, Movement>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    pub protected_movements: BTreeSet<MovementID>,
    pub yield_movements: BTreeSet<MovementID>,
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

    pub fn get_min_crossing_time(&self, idx: usize) -> Duration {
        let mut max_distance = Distance::meters(0.0);
        for movement in &self.stages[idx].protected_movements {
            if movement.crosswalk {
                max_distance =
                    max_distance.max(self.movements.get(&movement).unwrap().geom.length());
            }
        }
        let time = max_distance / CROSSWALK_PACE;
        assert!(time >= Duration::ZERO);
        // Round up because it is converted to a usize elsewhere
        Duration::seconds(time.inner_seconds().ceil())
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        // Does the assignment cover the correct set of movements?
        let expected_movements: BTreeSet<MovementID> = self.movements.keys().cloned().collect();
        let mut actual_movements: BTreeSet<MovementID> = BTreeSet::new();
        for stage in &self.stages {
            actual_movements.extend(stage.protected_movements.iter());
            actual_movements.extend(stage.yield_movements.iter());
        }
        if expected_movements != actual_movements {
            return Err(format!(
                "Traffic signal assignment for {} broken. Missing {:?}, contains irrelevant {:?}",
                self.id,
                expected_movements
                    .difference(&actual_movements)
                    .cloned()
                    .collect::<Vec<_>>(),
                actual_movements
                    .difference(&expected_movements)
                    .cloned()
                    .collect::<Vec<_>>()
            ));
        }
        let mut stage_index = 0;
        for stage in &self.stages {
            // Do any of the priority movements in one stage conflict?
            for m1 in stage.protected_movements.iter().map(|m| &self.movements[m]) {
                for m2 in stage.protected_movements.iter().map(|m| &self.movements[m]) {
                    if m1.conflicts_with(m2) {
                        return Err(format!(
                            "Traffic signal has conflicting protected movements in one \
                             stage:\n{:?}\n\n{:?}",
                            m1, m2
                        ));
                    }
                }
            }

            // Do any of the crosswalks yield?
            for m in stage.yield_movements.iter().map(|m| &self.movements[m]) {
                assert!(m.turn_type != TurnType::Crosswalk);
            }
            // Is there enough time in each stage to walk across the crosswalk
            let min_crossing_time = self.get_min_crossing_time(stage_index);
            if stage.phase_type.simple_duration() < min_crossing_time {
                return Err(format!(
                    "Traffic signal does not allow enough time in stage to complete the \
                     crosswalk\nStage Index{}\nStage : {:?}\nTime Required: {}\nTime Given: {}",
                    stage_index,
                    stage,
                    min_crossing_time,
                    stage.phase_type.simple_duration()
                ));
            }
            stage_index += 1;
        }
        Ok(())
    }

    // Returns true if this did anything
    pub fn convert_to_ped_scramble(&mut self) -> bool {
        let orig = self.clone();

        let mut all_walk_stage = Stage::new();
        for m in self.movements.values() {
            if m.turn_type == TurnType::Crosswalk {
                all_walk_stage.edit_movement(m, TurnPriority::Protected);
            }
        }

        // Remove Crosswalk movements from existing stages.
        let mut replaced = std::mem::replace(&mut self.stages, Vec::new());
        let mut has_all_walk = false;
        for stage in replaced.iter_mut() {
            if !has_all_walk && stage == &all_walk_stage {
                has_all_walk = true;
                continue;
            }

            // Crosswalks are only in protected_movements.
            retain_btreeset(&mut stage.protected_movements, |m| {
                self.movements[m].turn_type != TurnType::Crosswalk
            });

            // Blindly try to promote yield movements to protected, now that crosswalks are gone.
            let mut promoted = Vec::new();
            for m in &stage.yield_movements {
                if stage.could_be_protected(*m, &self.movements) {
                    stage.protected_movements.insert(*m);
                    promoted.push(*m);
                }
            }
            for m in promoted {
                stage.yield_movements.remove(&m);
            }
        }
        self.stages = replaced;

        if !has_all_walk {
            self.stages.push(all_walk_stage);
        }
        self != &orig
    }

    pub fn turn_to_movement(&self, turn: TurnID) -> MovementID {
        if let Some(m) = self.movements.values().find(|m| m.members.contains(&turn)) {
            m.id
        } else {
            panic!("{} doesn't belong to any movements in {}", turn, self.id)
        }
    }

    pub fn missing_turns(&self) -> BTreeSet<MovementID> {
        let mut missing: BTreeSet<MovementID> = self.movements.keys().cloned().collect();
        for stage in &self.stages {
            for m in &stage.protected_movements {
                missing.remove(m);
            }
            for m in &stage.yield_movements {
                missing.remove(m);
            }
        }
        missing
    }

    pub fn compressed_id(&self, turn: TurnID) -> CompressedMovementID {
        for (idx, m) in self.movements.values().enumerate() {
            if m.members.contains(&turn) {
                return CompressedMovementID {
                    i: self.id,
                    idx: u8::try_from(idx).unwrap(),
                };
            }
        }
        panic!(
            "{} doesn't belong to any turn movements in {}",
            turn, self.id
        )
    }
}

impl Stage {
    pub fn new() -> Stage {
        Stage {
            protected_movements: BTreeSet::new(),
            yield_movements: BTreeSet::new(),
            // TODO Set a default
            phase_type: PhaseType::Fixed(Duration::seconds(30.0)),
        }
    }

    pub fn could_be_protected(
        &self,
        m1: MovementID,
        movements: &BTreeMap<MovementID, Movement>,
    ) -> bool {
        let movement1 = &movements[&m1];
        for m2 in &self.protected_movements {
            if m1 == *m2 || movement1.conflicts_with(&movements[m2]) {
                return false;
            }
        }
        true
    }

    pub fn get_priority_of_turn(&self, t: TurnID, parent: &ControlTrafficSignal) -> TurnPriority {
        self.get_priority_of_movement(parent.turn_to_movement(t))
    }

    pub fn get_priority_of_movement(&self, m: MovementID) -> TurnPriority {
        if self.protected_movements.contains(&m) {
            TurnPriority::Protected
        } else if self.yield_movements.contains(&m) {
            TurnPriority::Yield
        } else {
            TurnPriority::Banned
        }
    }

    pub fn edit_movement(&mut self, g: &Movement, pri: TurnPriority) {
        let mut ids = vec![g.id];
        if g.turn_type == TurnType::Crosswalk {
            ids.push(MovementID {
                from: g.id.to,
                to: g.id.from,
                parent: g.id.parent,
                crosswalk: true,
            });
            self.enforce_minimum_crosswalk_time(g);
        }
        for id in ids {
            self.protected_movements.remove(&id);
            self.yield_movements.remove(&id);
            if pri == TurnPriority::Protected {
                self.protected_movements.insert(id);
            } else if pri == TurnPriority::Yield {
                self.yield_movements.insert(id);
            }
        }
    }
    pub fn enforce_minimum_crosswalk_time(&mut self, movement: &Movement) {
        // Round up to an int, because it is exported as a usize
        let time = Duration::seconds(
            (movement.geom.length() / CROSSWALK_PACE)
                .inner_seconds()
                .ceil(),
        );
        if time > self.phase_type.simple_duration() {
            self.phase_type = match self.phase_type {
                PhaseType::Adaptive(_) => Adaptive(time),
                PhaseType::Fixed(_) => Fixed(time),
            };
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
                        .protected_movements
                        .iter()
                        .map(|t| export_movement(t, map))
                        .collect(),
                    permitted_turns: s
                        .yield_movements
                        .iter()
                        .map(|t| export_movement(t, map))
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

    pub(crate) fn import(
        raw: seattle_traffic_signals::TrafficSignal,
        id: IntersectionID,
        map: &Map,
    ) -> Result<ControlTrafficSignal, String> {
        let mut stages = Vec::new();
        for s in raw.phases {
            let num_protected = s.protected_turns.len();
            let num_permitted = s.permitted_turns.len();
            let protected_movements = s
                .protected_turns
                .into_iter()
                .filter_map(|t| import_movement(t, map))
                .collect::<BTreeSet<_>>();
            let yield_movements = s
                .permitted_turns
                .into_iter()
                .filter_map(|t| import_movement(t, map))
                .collect::<BTreeSet<_>>();
            if protected_movements.len() == num_protected && yield_movements.len() == num_permitted
            {
                stages.push(Stage {
                    protected_movements,
                    yield_movements,
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
                    "Failed to import some of the movements for {}",
                    raw.intersection_osm_node_id
                ));
            }
        }
        let ts = ControlTrafficSignal {
            id,
            stages,
            offset: Duration::seconds(raw.offset_seconds as f64),
            movements: Movement::for_i(id, map).unwrap(),
        };
        ts.validate()?;
        Ok(ts)
    }
}

fn export_movement(id: &MovementID, map: &Map) -> seattle_traffic_signals::Turn {
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

fn import_movement(id: seattle_traffic_signals::Turn, map: &Map) -> Option<MovementID> {
    Some(MovementID {
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
