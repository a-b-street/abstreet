use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap};

use crate::{
    osm, Direction, DrivingSide, IntersectionID, LaneID, Map, RoadID, TurnID, TurnPriority,
    TurnType,
};

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
    pub lane_closest_to_edge: LaneID,
    pub must_stop: bool,
}

impl ControlStopSign {
    pub fn new(map: &Map, id: IntersectionID) -> ControlStopSign {
        let mut ss = ControlStopSign {
            id,
            roads: BTreeMap::new(),
        };
        for r in &map.get_i(id).roads {
            let r = map.get_r(*r);
            let want_dir = if r.dst_i == id {
                Direction::Fwd
            } else {
                Direction::Back
            };
            let travel_lanes: Vec<LaneID> = r
                .lanes_ltr()
                .into_iter()
                .filter_map(|(id, dir, lt)| {
                    if dir == want_dir && lt.is_for_moving_vehicles() {
                        Some(id)
                    } else {
                        None
                    }
                })
                .collect();
            if !travel_lanes.is_empty() {
                let lane_closest_to_edge = if (map.get_config().driving_side == DrivingSide::Right)
                    == (want_dir == Direction::Fwd)
                {
                    *travel_lanes.last().unwrap()
                } else {
                    travel_lanes[0]
                };
                ss.roads.insert(
                    r.id,
                    RoadWithStopSign {
                        lane_closest_to_edge,
                        must_stop: false,
                    },
                );
            }
        }

        // Degenerate roads and deadends don't need any stop signs. But be careful with
        // counting the number of roads; a roundabout with 3 might only have 2 in ss.roads, because
        // one is outgoing. Nonetheless, we want to consider stop signs for it.
        if map.get_i(id).roads.len() <= 2 {
            return ss;
        }
        if map.get_i(id).is_cycleway(map) {
            // Two cyclepaths intersecting can just yield.
            return ss;
        }

        // Rank each road based on OSM highway type, and additionally treat cycleways as lower
        // priority than local roads. (Sad but typical reality.) Prioritize roundabouts, so they
        // clear out faster than people enter them.
        let mut rank: HashMap<RoadID, (osm::RoadRank, usize)> = HashMap::new();
        for r in ss.roads.keys() {
            let r = map.get_r(*r);
            // Lower number is lower priority
            let priority = if r.is_cycleway() {
                0
            } else if r.osm_tags.is("junction", "roundabout") {
                2
            } else {
                1
            };
            rank.insert(r.id, (r.get_rank(), priority));
        }
        let mut ranks = rank.values().cloned().collect::<Vec<_>>();
        ranks.sort();
        ranks.dedup();
        // Highest rank is first
        ranks.reverse();

        // If all roads have the same rank, all-way stop. Otherwise, everything stops except the
        // highest-priority roads.
        for (r, cfg) in ss.roads.iter_mut() {
            if ranks.len() == 1 || rank[r] != ranks[0] {
                // Don't stop in the middle of something that's likely actually an intersection.
                if !map.get_r(*r).is_extremely_short() {
                    cfg.must_stop = true;
                }
            }
        }
        ss
    }

    /// Get the priority of a turn according to the stop sign -- either protected or yield, never
    /// banned.
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
