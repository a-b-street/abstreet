use crate::{IntersectionID, LaneID, Map, RoadID, TurnID, TurnPriority, TurnType};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

// TODO These are old notes, they don't reflect current reality. But some of the ideas here should
// be implemented, so keeping them...
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
//    - "Higher priority" has two cases -- stop sign road always yields to a non-stop sign road. But
//      also a non-stop sign road yields to another non-stop sign road. In other words, left turns
//      yield to straight and ideally, lane-changing yields to straight too.
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
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub roads: BTreeMap<RoadID, RoadWithStopSign>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RoadWithStopSign {
    pub rightmost_lane: LaneID,
    pub must_stop: bool,
}

impl ControlStopSign {
    pub fn new(map: &Map, id: IntersectionID) -> ControlStopSign {
        let mut ss = ControlStopSign {
            id,
            roads: BTreeMap::new(),
        };
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
                        rightmost_lane: *travel_lanes.last().unwrap(),
                        must_stop: false,
                    },
                );
            }
        }

        if ss.roads.len() <= 2 {
            // Degenerate roads and deadends don't need any stop signs.
            return ss;
        }

        // What's the rank of each road?
        let mut rank: HashMap<RoadID, usize> = HashMap::new();
        for r in ss.roads.keys() {
            rank.insert(*r, map.get_r(*r).get_rank());
        }
        let mut ranks: Vec<usize> = rank.values().cloned().collect();
        ranks.sort();
        ranks.dedup();
        // Highest rank is first
        ranks.reverse();

        // If all roads have the same rank, all-way stop. Otherwise, everything stops except the
        // highest-priority roads.
        for (r, cfg) in ss.roads.iter_mut() {
            if ranks.len() == 1 || rank[r] != ranks[0] {
                cfg.must_stop = true;
            }
        }
        ss
    }

    // TODO Or cache
    pub fn get_priority(&self, turn: TurnID, map: &Map) -> TurnPriority {
        match map.get_t(turn).turn_type {
            TurnType::SharedSidewalkCorner => TurnPriority::Protected,
            // TODO This actually feels like a policy bit that should be flippable.
            TurnType::Crosswalk => TurnPriority::Protected,
            _ => {
                if self.roads[&map.get_l(turn.src).parent].must_stop {
                    TurnPriority::Yield
                } else {
                    TurnPriority::Protected
                }
            }
        }
    }

    pub fn flip_sign(&mut self, r: RoadID) {
        let ss = self.roads.get_mut(&r).unwrap();
        ss.must_stop = !ss.must_stop;
    }
}
