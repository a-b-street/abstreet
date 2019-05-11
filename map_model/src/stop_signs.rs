use crate::{IntersectionID, LaneID, Map, RoadID, TurnID, TurnPriority, TurnType};
use abstutil::{deserialize_btreemap, serialize_btreemap, Error, Timer, Warn};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

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
        /*self.turns
            .iter()
            .find(|(turn, pri)| turn.src == lane && **pri <= TurnPriority::Stop)
            .is_none()*/
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
}

fn smart_assignment(map: &Map, id: IntersectionID) -> Warn<ControlStopSign> {
    if map.get_i(id).roads.len() <= 2 {
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
        let r = map.get_parent(*l);
        let rank = if let Some(highway) = r.osm_tags.get("highway") {
            match highway.as_ref() {
                "motorway" => 20,
                "motorway_link" => 19,

                "trunk" => 17,
                "trunk_link" => 16,

                "primary" => 15,
                "primary_link" => 14,

                "secondary" => 13,
                "secondary_link" => 12,

                "tertiary" => 10,
                "tertiary_link" => 9,

                "residential" => 5,

                "footway" => 1,

                "unclassified" => 0,
                "road" => 0,
                _ => panic!("Unknown OSM highway {}", highway),
            }
        } else {
            0
        };
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
        if rank_per_incoming_lane[&t.src] == highest_rank {
            // If it's the highest rank road, prioritize all non-left turns (if possible) and make
            // other turns yield.
            if map.get_t(*t).turn_type != TurnType::Left && ss.could_be_priority_turn(*t, map) {
                ss.turns.insert(*t, TurnPriority::Priority);
            } else {
                ss.turns.insert(*t, TurnPriority::Yield);
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
        ss.turns.insert(*t, TurnPriority::Stop);
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
