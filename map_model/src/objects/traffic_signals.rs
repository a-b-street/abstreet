use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Distance, Duration, Speed};

use crate::make::traffic_signals::get_possible_policies;
use crate::raw::OriginalRoad;
use crate::{
    osm, CompressedMovementID, DirectedRoadID, Direction, IntersectionID, Map, Movement,
    MovementID, RoadID, TurnID, TurnPriority, TurnType,
};

// The pace to use for crosswalk pace in m/s
// https://en.wikipedia.org/wiki/Preferred_walking_speed
const CROSSWALK_PACE: Speed = Speed::const_meters_per_second(1.4);

/// A traffic signal consists of a sequence of Stages that repeat in a cycle. Most Stages last for a
/// fixed duration. During a single Stage, some movements are protected (can proceed with the
/// highest priority), while others are permitted (have to yield before proceeding).
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
    // https://github.com/a-b-street/abstreet/pull/298 anyway
    pub stage_type: StageType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StageType {
    Fixed(Duration),
    /// Minimum is the minimum duration, 0 allows cycle to be skipped if no demand.
    /// Delay is the elapsed time with no demand that ends a cycle.
    /// Additional is the additional duration for an extended cycle.
    Variable(Duration, Duration, Duration),
}

impl StageType {
    // TODO Maybe don't have this; force callers to acknowledge different policies
    pub fn simple_duration(&self) -> Duration {
        match self {
            StageType::Fixed(d) => *d,
            StageType::Variable(duration, _, _) => *duration,
        }
    }
}

impl ControlTrafficSignal {
    pub fn new(map: &Map, id: IntersectionID) -> ControlTrafficSignal {
        let mut policies = ControlTrafficSignal::get_possible_policies(map, id);
        if policies.len() == 1 {
            warn!("Falling back to greedy_assignment for {}", id);
        }
        policies.remove(0).1
    }

    /// Only call this variant while importing the map, to enforce that baked-in signal config is
    /// valid.
    pub(crate) fn validating_new(map: &Map, id: IntersectionID) -> ControlTrafficSignal {
        let mut policies = get_possible_policies(map, id, true);
        if policies.len() == 1 {
            warn!("Falling back to greedy_assignment for {}", id);
        }
        policies.remove(0).1
    }

    pub fn get_possible_policies(
        map: &Map,
        id: IntersectionID,
    ) -> Vec<(String, ControlTrafficSignal)> {
        // This method is called publicly while editing the map, so don't enforce valid baked-in
        // signal config.
        get_possible_policies(map, id, false)
    }

    pub fn get_min_crossing_time(&self, idx: usize) -> Duration {
        let mut max_distance = Distance::meters(0.0);
        for movement in &self.stages[idx].protected_movements {
            if movement.crosswalk {
                max_distance =
                    max_distance.max(self.movements.get(movement).unwrap().geom.length());
            }
        }
        let time = max_distance / CROSSWALK_PACE;
        assert!(time >= Duration::ZERO);
        // Round up because it is converted to a usize elsewhere
        Duration::seconds(time.inner_seconds().ceil())
    }

    pub fn validate(&self) -> Result<()> {
        // Does the assignment cover the correct set of movements?
        let expected_movements: BTreeSet<MovementID> = self.movements.keys().cloned().collect();
        let mut actual_movements: BTreeSet<MovementID> = BTreeSet::new();
        for stage in &self.stages {
            actual_movements.extend(stage.protected_movements.iter());
            actual_movements.extend(stage.yield_movements.iter());
        }
        if expected_movements != actual_movements {
            bail!(
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
            );
        }
        for (stage_index, stage) in self.stages.iter().enumerate() {
            // Do any of the priority movements in one stage conflict?
            for m1 in stage.protected_movements.iter().map(|m| &self.movements[m]) {
                for m2 in stage.protected_movements.iter().map(|m| &self.movements[m]) {
                    if m1.conflicts_with(m2) {
                        bail!(
                            "Traffic signal has conflicting protected movements in one \
                             stage:\n{:?}\n\n{:?}",
                            m1,
                            m2
                        );
                    }
                }
            }

            // Do any of the crosswalks yield?
            for m in stage.yield_movements.iter().map(|m| &self.movements[m]) {
                assert!(m.turn_type != TurnType::Crosswalk);
            }
            // Is there enough time in each stage to walk across the crosswalk
            let min_crossing_time = self.get_min_crossing_time(stage_index);
            if stage.stage_type.simple_duration() < min_crossing_time {
                bail!(
                    "Traffic signal does not allow enough time in stage to complete the \
                     crosswalk\nStage Index{}\nStage : {:?}\nTime Required: {}\nTime Given: {}",
                    stage_index,
                    stage,
                    min_crossing_time,
                    stage.stage_type.simple_duration()
                );
            }
        }
        Ok(())
    }

    /// Move crosswalks from stages, adding them to an all-walk as last stage. This may promote
    /// yields to protected. True is returned if any stages were added or modified.
    pub fn convert_to_ped_scramble(&mut self) -> bool {
        self.internal_convert_to_ped_scramble(true)
    }
    /// Move crosswalks from stages, adding them to an all-walk as last stage. This does not promote
    /// yields to protected. True is returned if any stages were added or modified.
    pub fn convert_to_ped_scramble_without_promotion(&mut self) -> bool {
        self.internal_convert_to_ped_scramble(false)
    }

    fn internal_convert_to_ped_scramble(&mut self, promote_yield_to_protected: bool) -> bool {
        let orig = self.clone();

        let mut all_walk_stage = Stage::new();
        for m in self.movements.values() {
            if m.turn_type == TurnType::Crosswalk {
                all_walk_stage.edit_movement(m, TurnPriority::Protected);
            }
        }

        // Remove Crosswalk movements from existing stages.
        let mut replaced = std::mem::take(&mut self.stages);
        let mut has_all_walk = false;
        for stage in replaced.iter_mut() {
            if !has_all_walk && stage == &all_walk_stage {
                has_all_walk = true;
                continue;
            }

            // Crosswalks are only in protected_movements.
            stage
                .protected_movements
                .retain(|m| self.movements[m].turn_type != TurnType::Crosswalk);
            if promote_yield_to_protected {
                // Blindly try to promote yield movements to protected, now that crosswalks are
                // gone.
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
        }
        self.stages = replaced;

        if !has_all_walk {
            self.stages.push(all_walk_stage);
        }
        self != &orig
    }

    /// Modifies the fixed timing of all stages, applying either a major or minor duration,
    /// depending on the relative rank of the roads involved in the intersection. If this
    /// transformation couldn't be applied, returns an error. Even if an error is returned, the
    /// signal may have been changed -- so only call this on a cloned signal.
    pub fn adjust_major_minor_timing(
        &mut self,
        major: Duration,
        minor: Duration,
        map: &Map,
    ) -> Result<()> {
        if self.stages.len() != 2 {
            bail!("This intersection doesn't have 2 stages.");
        }

        // What's the rank of each road?
        let mut rank_per_road: BTreeMap<RoadID, usize> = BTreeMap::new();
        for r in &map.get_i(self.id).roads {
            rank_per_road.insert(*r, map.get_r(*r).get_detailed_rank());
        }
        let mut ranks: Vec<usize> = rank_per_road.values().cloned().collect();
        ranks.sort_unstable();
        ranks.dedup();
        if ranks.len() == 1 {
            bail!("This intersection doesn't have major/minor roads; they're all the same rank.");
        }
        let highest_rank = ranks.pop().unwrap();

        // Try to apply the transformation
        let orig = self.clone();
        for stage in &mut self.stages {
            match stage.stage_type {
                StageType::Fixed(_) => {}
                _ => bail!("This intersection doesn't use fixed timing."),
            }
            // Ignoring crosswalks, do any of the turns come from a major road?
            if stage
                .protected_movements
                .iter()
                .any(|m| !m.crosswalk && highest_rank == rank_per_road[&m.from.id])
            {
                stage.stage_type = StageType::Fixed(major);
            } else {
                stage.stage_type = StageType::Fixed(minor);
            }
        }

        if self.simple_cycle_duration() != major + minor {
            bail!("This intersection didn't already group major/minor roads together.");
        }

        if self == &orig {
            bail!("This change had no effect.");
        }

        Ok(())
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

    /// How long a full cycle of the signal lasts, assuming no actuated timings.
    pub fn simple_cycle_duration(&self) -> Duration {
        let mut total = Duration::ZERO;
        for s in &self.stages {
            total += s.stage_type.simple_duration();
        }
        total
    }
}

impl Stage {
    pub fn new() -> Stage {
        Stage {
            protected_movements: BTreeSet::new(),
            yield_movements: BTreeSet::new(),
            // TODO Set a default
            stage_type: StageType::Fixed(Duration::seconds(30.0)),
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
        if time > self.stage_type.simple_duration() {
            self.stage_type = match self.stage_type {
                StageType::Fixed(_) => StageType::Fixed(time),
                StageType::Variable(_, delay, additional) => {
                    StageType::Variable(time, delay, additional)
                }
            };
        }
    }

    // A trivial function that returns max crosswalk time if the stage is just crosswalks.
    pub fn max_crosswalk_time(
        &self,
        movements: &BTreeMap<MovementID, Movement>,
    ) -> Option<Duration> {
        let mut max_distance = Distance::const_meters(0.0);
        for m in &self.protected_movements {
            if m.crosswalk {
                max_distance = max_distance.max(movements.get(m).unwrap().geom.length());
            } else {
                return None;
            }
        }
        if max_distance > Distance::const_meters(0.0) {
            let time = max_distance / CROSSWALK_PACE;
            assert!(time >= Duration::ZERO);
            // Round up because it is converted to a usize elsewhere
            Some(Duration::seconds(time.inner_seconds().ceil()))
        } else {
            None
        }
    }
}

impl ControlTrafficSignal {
    pub fn export(&self, map: &Map) -> traffic_signal_data::TrafficSignal {
        traffic_signal_data::TrafficSignal {
            intersection_osm_node_id: map.get_i(self.id).orig_id.0,
            plans: vec![traffic_signal_data::Plan {
                start_time_seconds: 0,
                stages: self
                    .stages
                    .iter()
                    .map(|s| traffic_signal_data::Stage {
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
                        stage_type: match s.stage_type {
                            StageType::Fixed(d) => {
                                traffic_signal_data::StageType::Fixed(d.inner_seconds() as usize)
                            }
                            StageType::Variable(min, delay, additional) => {
                                traffic_signal_data::StageType::Variable(
                                    min.inner_seconds() as usize,
                                    delay.inner_seconds() as usize,
                                    additional.inner_seconds() as usize,
                                )
                            }
                        },
                    })
                    .collect(),
                offset_seconds: self.offset.inner_seconds() as usize,
            }],
        }
    }

    pub(crate) fn import(
        mut raw: traffic_signal_data::TrafficSignal,
        id: IntersectionID,
        map: &Map,
    ) -> Result<ControlTrafficSignal> {
        // TODO Only import the first plan. Will import all of them later.
        let plan = raw.plans.remove(0);
        let mut stages = Vec::new();
        for s in plan.stages {
            let mut errors = Vec::new();
            let mut protected_movements = BTreeSet::new();
            for t in s.protected_turns {
                match import_movement(t, map) {
                    Ok(mvmnt) => {
                        protected_movements.insert(mvmnt);
                    }
                    Err(err) => {
                        errors.push(err.to_string());
                    }
                }
            }
            let mut permitted_movements = BTreeSet::new();
            for t in s.permitted_turns {
                match import_movement(t, map) {
                    Ok(mvmnt) => {
                        permitted_movements.insert(mvmnt);
                    }
                    Err(err) => {
                        errors.push(err.to_string());
                    }
                }
            }
            if errors.is_empty() {
                stages.push(Stage {
                    protected_movements,
                    yield_movements: permitted_movements,
                    stage_type: match s.stage_type {
                        traffic_signal_data::StageType::Fixed(d) => {
                            StageType::Fixed(Duration::seconds(d as f64))
                        }
                        traffic_signal_data::StageType::Variable(min, delay, additional) => {
                            StageType::Variable(
                                Duration::seconds(min as f64),
                                Duration::seconds(delay as f64),
                                Duration::seconds(additional as f64),
                            )
                        }
                    },
                });
            } else {
                bail!("{}", errors.join("; "));
            }
        }
        let ts = ControlTrafficSignal {
            id,
            stages,
            offset: Duration::seconds(plan.offset_seconds as f64),
            movements: Movement::for_i(id, map).unwrap(),
        };
        ts.validate()?;
        Ok(ts)
    }
}

fn export_movement(id: &MovementID, map: &Map) -> traffic_signal_data::Turn {
    let from = map.get_r(id.from.id).orig_id;
    let to = map.get_r(id.to.id).orig_id;

    traffic_signal_data::Turn {
        from: traffic_signal_data::DirectedRoad {
            osm_way_id: from.osm_way_id.0,
            osm_node1: from.i1.0,
            osm_node2: from.i2.0,
            is_forwards: id.from.dir == Direction::Fwd,
        },
        to: traffic_signal_data::DirectedRoad {
            osm_way_id: to.osm_way_id.0,
            osm_node1: to.i1.0,
            osm_node2: to.i2.0,
            is_forwards: id.to.dir == Direction::Fwd,
        },
        intersection_osm_node_id: map.get_i(id.parent).orig_id.0,
        is_crosswalk: id.crosswalk,
    }
}

fn import_movement(id: traffic_signal_data::Turn, map: &Map) -> Result<MovementID> {
    Ok(MovementID {
        from: find_r(id.from, map)?,
        to: find_r(id.to, map)?,
        parent: map.find_i_by_osm_id(osm::NodeID(id.intersection_osm_node_id))?,
        crosswalk: id.is_crosswalk,
    })
}

fn find_r(id: traffic_signal_data::DirectedRoad, map: &Map) -> Result<DirectedRoadID> {
    Ok(DirectedRoadID {
        id: map.find_r_by_osm_id(OriginalRoad::new(
            id.osm_way_id,
            (id.osm_node1, id.osm_node2),
        ))?,
        dir: if id.is_forwards {
            Direction::Fwd
        } else {
            Direction::Back
        },
    })
}
