use crate::{IntersectionID, LaneID, Map, RoadID, TurnID, TurnPriority, TurnType};
use abstutil::{deserialize_btreemap, serialize_btreemap, Error, Timer, Warn};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

// TODO Some of these are probably old notes:
// 1) Pedestrians always have right-of-way. (for now -- should be toggleable later)
// 2) Incoming roads without a stop sign have priority over roads with a sign.
// 3) Agents with a stop sign have to actually wait some amount of time before starting the turn.
// 4) Before starting any turn, an agent should make sure it can complete the turn without making a
//    higher-priority agent have to wait.
//    - "Complete" the turn just means the optimistic "length / max_speed" calculation -- if they
//      queue behind slow cars upstream a bit, blocking the intersection a little bit is nice and
//      realistic.
//    - The higher priority agent might not even be at the intersection yet! This'll be a little
//      harder to implement.
//    - "Higher priority" has two cases -- stop sign road always yields to a non-stop sign road.
//      But also a non-stop sign road yields to another non-stop sign road. In other words, left
//      turns yield to straight and ideally, lane-changing yields to straight too.
//    - So there still is a notion of turn priorities -- priority (can never conflict with another
//      priority), yield (can't impede a priority turn), stop (has to pause and can't impede a
//      priority or yield turn). But I don't think we want to really depict this...
// 5) Rule 4 gives us a notion of roads that conflict -- or actually, do we even need it? No! An
//    intersection with no stop signs at all means everyone yields. An intersection with all stop
//    signs means everyone pauses before proceeding.
// 6) Additionally, individual turns can be banned completely.
//    - Even though letting players manipulate this could make parts of the map unreachable?

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlStopSign {
    pub id: IntersectionID,
    // Turns may be present here as Banned.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub turns: BTreeMap<TurnID, TurnPriority>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub roads: BTreeMap<RoadID, RoadWithStopSign>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RoadWithStopSign {
    pub travel_lanes: Vec<LaneID>,
    pub enabled: bool,
}

impl ControlStopSign {
    pub fn new(map: &Map, id: IntersectionID, timer: &mut Timer) -> ControlStopSign {
        let mut ss = smart_assignment(map, id).get(timer);
        ss.validate(map).unwrap().get(timer);

        for r in &map.get_i(id).roads {
            let travel_lanes: Vec<LaneID> = map
                .get_r(*r)
                .incoming_lanes(id)
                .iter()
                .filter_map(|(id, lt)| {
                    if lt.is_for_moving_vehicles() {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            if !travel_lanes.is_empty() {
                ss.roads.insert(
                    *r,
                    RoadWithStopSign {
                        travel_lanes,
                        enabled: false,
                    },
                );
            }
        }
        ss.recalculate_stop_signs(map);

        ss
    }

    pub fn get_priority(&self, turn: TurnID) -> TurnPriority {
        self.turns[&turn]
    }

    pub fn could_be_priority_turn(&self, id: TurnID, map: &Map) -> bool {
        for (t, pri) in &self.turns {
            if *pri == TurnPriority::Priority && map.get_t(id).conflicts_with(map.get_t(*t)) {
                return false;
            }
        }
        true
    }

    pub fn lane_has_stop_sign(&self, lane: LaneID) -> bool {
        for ss in self.roads.values() {
            if ss.travel_lanes.contains(&lane) {
                return ss.enabled;
            }
        }
        false
    }

    // Returns both errors and warnings.
    fn validate(&self, map: &Map) -> Result<Warn<()>, Error> {
        let mut warnings = Vec::new();

        // Does the assignment cover the correct set of turns?
        let all_turns = &map.get_i(self.id).turns;
        // TODO Panic after stabilizing merged intersection issues.
        if self.turns.len() != all_turns.len() {
            warnings.push(format!(
                "Stop sign for {} has {} turns but should have {}",
                self.id,
                self.turns.len(),
                all_turns.len()
            ));
        }
        for t in all_turns {
            if !self.turns.contains_key(t) {
                warnings.push(format!("Stop sign for {} is missing {}", self.id, t));
            }
            // Are all of the SharedSidewalkCorner prioritized?
            if map.get_t(*t).turn_type == TurnType::SharedSidewalkCorner {
                assert_eq!(self.turns[t], TurnPriority::Priority);
            }
        }

        // Do any of the priority turns conflict?
        let priority_turns: Vec<TurnID> = self
            .turns
            .iter()
            .filter_map(|(turn, pri)| {
                if *pri == TurnPriority::Priority {
                    Some(*turn)
                } else {
                    None
                }
            })
            .collect();
        for t1 in &priority_turns {
            for t2 in &priority_turns {
                if map.get_t(*t1).conflicts_with(map.get_t(*t2)) {
                    return Err(Error::new(format!(
                        "Stop sign has conflicting priority turns {:?} and {:?}",
                        t1, t2
                    )));
                }
            }
        }

        Ok(Warn::empty_warnings(warnings))
    }

    pub fn change(&mut self, t: TurnID, pri: TurnPriority, map: &Map) {
        let turn = map.get_t(t);
        self.turns.insert(t, pri);
        if turn.turn_type == TurnType::Crosswalk {
            for id in &turn.other_crosswalk_ids {
                self.turns.insert(*id, pri);
            }
        }
        self.recalculate_stop_signs(map);
    }

    // TODO Actually want to recalculate individual turn priorities for everything when anything
    // changes! I think the base data model needs to become 'roads' with some Banned overrides.
    pub fn flip_sign(&mut self, r: RoadID, map: &Map) {
        let ss = self.roads.get_mut(&r).unwrap();
        ss.enabled = !ss.enabled;
        let new_pri = if ss.enabled {
            TurnPriority::Stop
        } else {
            TurnPriority::Yield
        };
        for l in ss.travel_lanes.clone() {
            for (turn, _) in map.get_next_turns_and_lanes(l, self.id) {
                self.turns.insert(turn.id, new_pri);

                // Upgrade some turns to priority
                if new_pri == TurnPriority::Yield && self.could_be_priority_turn(turn.id, map) {
                    match turn.turn_type {
                        TurnType::Straight | TurnType::Right | TurnType::Crosswalk => {
                            self.turns.insert(turn.id, TurnPriority::Priority);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn recalculate_stop_signs(&mut self, map: &Map) {
        for ss in self.roads.values_mut() {
            ss.enabled = false;
            for l in &ss.travel_lanes {
                for (turn, _) in map.get_next_turns_and_lanes(*l, self.id) {
                    match self.turns[&turn.id] {
                        TurnPriority::Stop | TurnPriority::Banned => {
                            ss.enabled = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn smart_assignment(map: &Map, id: IntersectionID) -> Warn<ControlStopSign> {
    // Count the number of roads with incoming lanes to determine degenerate/deadends. Might have
    // one incoming road to two outgoing. Don't count sidewalks as incoming; crosswalks always
    // yield anyway.
    let mut incoming_roads: HashSet<RoadID> = HashSet::new();
    for l in &map.get_i(id).incoming_lanes {
        if map.get_l(*l).lane_type.is_for_moving_vehicles() {
            incoming_roads.insert(map.get_l(*l).parent);
        }
    }
    if incoming_roads.len() <= 2 {
        return for_degenerate_and_deadend(map, id);
    }

    // Higher numbers are higher rank roads
    let mut rank_per_incoming_lane: HashMap<LaneID, usize> = HashMap::new();
    let mut ranks: HashSet<usize> = HashSet::new();
    let mut highest_rank = 0;
    // TODO should just be incoming, but because of weirdness with sidewalks...
    for l in map
        .get_i(id)
        .incoming_lanes
        .iter()
        .chain(map.get_i(id).outgoing_lanes.iter())
    {
        let rank = map.get_parent(*l).get_rank();
        rank_per_incoming_lane.insert(*l, rank);
        highest_rank = highest_rank.max(rank);
        ranks.insert(rank);
    }
    if ranks.len() == 1 {
        return Warn::ok(all_way_stop(map, id));
    }

    let mut ss = ControlStopSign {
        id,
        turns: BTreeMap::new(),
        roads: BTreeMap::new(),
    };
    for t in &map.get_i(id).turns {
        if map.get_t(*t).turn_type == TurnType::SharedSidewalkCorner {
            ss.turns.insert(*t, TurnPriority::Priority);
        } else if rank_per_incoming_lane[&t.src] == highest_rank {
            // If it's the highest rank road, prioritize main turns and make others yield.
            ss.turns.insert(*t, TurnPriority::Yield);
            if ss.could_be_priority_turn(*t, map) {
                match map.get_t(*t).turn_type {
                    TurnType::Straight | TurnType::Right | TurnType::Crosswalk => {
                        ss.turns.insert(*t, TurnPriority::Priority);
                    }
                    _ => {}
                }
            }
        } else {
            // Lower rank roads have to stop.
            ss.turns.insert(*t, TurnPriority::Stop);
        }
    }
    Warn::ok(ss)
}

fn all_way_stop(map: &Map, id: IntersectionID) -> ControlStopSign {
    let mut ss = ControlStopSign {
        id,
        turns: BTreeMap::new(),
        roads: BTreeMap::new(),
    };
    for t in &map.get_i(id).turns {
        if map.get_t(*t).turn_type == TurnType::SharedSidewalkCorner {
            ss.turns.insert(*t, TurnPriority::Priority);
        } else {
            ss.turns.insert(*t, TurnPriority::Stop);
        }
    }
    ss
}

fn for_degenerate_and_deadend(map: &Map, id: IntersectionID) -> Warn<ControlStopSign> {
    let mut ss = ControlStopSign {
        id,
        turns: BTreeMap::new(),
        roads: BTreeMap::new(),
    };
    for t in &map.get_i(id).turns {
        // Only the crosswalks should conflict with other turns.
        let priority = match map.get_t(*t).turn_type {
            TurnType::Crosswalk => TurnPriority::Stop,
            TurnType::LaneChangeLeft | TurnType::LaneChangeRight => TurnPriority::Yield,
            _ => TurnPriority::Priority,
        };
        ss.turns.insert(*t, priority);
    }

    // Due to a few observed issues (multiple driving lanes road (a temporary issue) and bad
    // intersection geometry), sometimes more turns conflict than really should. For now, just
    // detect and fallback to an all-way stop.
    if let Err(err) = ss.validate(map) {
        return Warn::warn(
            all_way_stop(map, id),
            format!("Giving up on for_degenerate_and_deadend({}): {}", id, err),
        );
    }

    Warn::ok(ss)
}
