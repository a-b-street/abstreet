//! To deal with complicated intersections and short roads in OSM, cluster intersections close
//! together and then calculate UberTurns that string together several turns.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::graphmap::UnGraphMap;
use serde::{Deserialize, Serialize};

use geom::{Distance, PolyLine};

use crate::{IntersectionID, LaneID, Map, TurnID};

/// This only applies to VehiclePathfinder; walking through these intersections is nothing special.
// TODO I haven't seen any cases yet with "interior" intersections. Some stuff might break.
pub struct IntersectionCluster {
    pub members: BTreeSet<IntersectionID>,
    pub uber_turns: Vec<UberTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UberTurn {
    pub path: Vec<TurnID>,
}

impl IntersectionCluster {
    pub fn find_all(map: &Map) -> Vec<IntersectionCluster> {
        // First autodetect based on traffic signals close together.
        let mut clusters = Vec::new();
        let mut seen_intersections = BTreeSet::new();
        for i in map.all_intersections() {
            if i.is_traffic_signal() && !seen_intersections.contains(&i.id) {
                if let Some(members) = IntersectionCluster::autodetect(i.id, map) {
                    seen_intersections.extend(members.clone());
                    // Discard any illegal movements
                    clusters.push(IntersectionCluster::new(members, map).0);
                }
            }
        }

        // Then look for intersections with complicated turn restrictions.
        let mut graph: UnGraphMap<IntersectionID, ()> = UnGraphMap::new();
        for from in map.all_roads() {
            for (via, _) in &from.complicated_turn_restrictions {
                // Each of these tells us 2 intersections to group together
                let r = map.get_r(*via);
                graph.add_edge(r.src_i, r.dst_i, ());
            }
        }
        for intersections in petgraph::algo::kosaraju_scc(&graph) {
            let members: BTreeSet<IntersectionID> = intersections.iter().cloned().collect();
            // Is there already a cluster covering everything?
            if clusters.iter().any(|ic| ic.members.is_subset(&members)) {
                continue;
            }

            // Do any existing clusters partly cover this one?
            let mut existing: Vec<&mut IntersectionCluster> = clusters
                .iter_mut()
                .filter(|ic| ic.members.intersection(&members).next().is_some())
                .collect();
            // None? Just add a new one.
            if existing.is_empty() {
                clusters.push(IntersectionCluster::new(members, map).0);
                continue;
            }

            if existing.len() == 1 {
                // Amend this existing one.
                let mut all_members = members;
                all_members.extend(existing[0].members.clone());
                *existing[0] = IntersectionCluster::new(all_members, map).0;
                continue;
            }

            // TODO Saw this is New Orleans
            println!(
                "Need a cluster containing {:?} for turn restrictions, but there's more than one \
                 existing cluster that partly covers it. Union them?",
                members
            );
            return Vec::new();
        }

        clusters
    }

    /// (legal, illegal)
    pub fn new(
        members: BTreeSet<IntersectionID>,
        map: &Map,
    ) -> (IntersectionCluster, IntersectionCluster) {
        // Find all entrances and exits through this group of intersections
        let mut entrances = Vec::new();
        let mut exits = BTreeSet::new();
        for i in &members {
            for turn in map.get_turns_in_intersection(*i) {
                if turn.between_sidewalks() {
                    continue;
                }
                if !members.contains(&map.get_l(turn.id.src).src_i) {
                    entrances.push(turn.id);
                }
                if !members.contains(&map.get_l(turn.id.dst).dst_i) {
                    exits.insert(turn.id);
                }
            }
        }

        // Find all paths between entrances and exits
        let mut uber_turns = Vec::new();
        for entrance in entrances {
            uber_turns.extend(flood(entrance, map, &exits));
        }

        // Filter illegal paths
        let mut all_restrictions = Vec::new();
        for from in map.all_roads() {
            for (via, to) in &from.complicated_turn_restrictions {
                all_restrictions.push((from.id, *via, *to));
            }
        }

        // Filter out the restricted ones!
        let mut illegal = Vec::new();
        uber_turns.retain(|ut| {
            let mut ok = true;
            for pair in ut.path.windows(2) {
                let r1 = map.get_l(pair[0].src).parent;
                let r2 = map.get_l(pair[0].dst).parent;
                let r3 = map.get_l(pair[1].dst).parent;
                if all_restrictions.contains(&(r1, r2, r3)) {
                    ok = false;
                    break;
                }
            }
            if ok {
                true
            } else {
                // TODO There's surely a method in Vec to do partition like this
                illegal.push(ut.clone());
                false
            }
        });

        (
            IntersectionCluster {
                members: members.clone(),
                uber_turns,
            },
            IntersectionCluster {
                members,
                uber_turns: illegal,
            },
        )
    }

    /// Find all other traffic signals "close" to one. Ignore stop sign intersections in between.
    pub fn autodetect(from: IntersectionID, map: &Map) -> Option<BTreeSet<IntersectionID>> {
        if !map.get_i(from).is_traffic_signal() {
            return None;
        }
        let threshold = Distance::meters(25.0);

        let mut found = BTreeSet::new();
        let mut queue = vec![from];

        while !queue.is_empty() {
            let i = map.get_i(queue.pop().unwrap());
            if found.contains(&i.id) {
                continue;
            }
            found.insert(i.id);
            for r in &i.roads {
                let r = map.get_r(*r);
                if r.center_pts.length() > threshold {
                    continue;
                }
                let other = if r.src_i == i.id { r.dst_i } else { r.src_i };
                if map.get_i(other).is_traffic_signal() {
                    queue.push(other);
                }
            }
        }
        if found.len() > 1 {
            Some(found)
        } else {
            None
        }
    }
}

fn flood(start: TurnID, map: &Map, exits: &BTreeSet<TurnID>) -> Vec<UberTurn> {
    if exits.contains(&start) {
        return vec![UberTurn { path: vec![start] }];
    }

    let mut results = Vec::new();
    let mut preds: BTreeMap<TurnID, TurnID> = BTreeMap::new();
    let mut queue = vec![start];

    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        for next in map.get_turns_from_lane(current.dst) {
            if preds.contains_key(&next.id) {
                continue;
            }
            preds.insert(next.id, current);
            if exits.contains(&next.id) {
                results.push(UberTurn {
                    path: trace_back(next.id, &preds),
                });
            } else {
                queue.push(next.id);
            }
        }
    }

    results
}

fn trace_back(end: TurnID, preds: &BTreeMap<TurnID, TurnID>) -> Vec<TurnID> {
    let mut path = vec![end];
    let mut current = end;
    loop {
        if let Some(prev) = preds.get(&current) {
            path.push(*prev);
            current = *prev;
        } else {
            path.reverse();
            return path;
        }
    }
}

impl UberTurn {
    pub fn entry(&self) -> LaneID {
        self.path[0].src
    }
    pub fn exit(&self) -> LaneID {
        self.path.last().unwrap().dst
    }

    pub fn geom(&self, map: &Map) -> PolyLine {
        let mut pl = map.get_t(self.path[0]).geom.clone();
        let mut first = true;
        for pair in self.path.windows(2) {
            if !first {
                pl = pl.must_extend(map.get_t(pair[0]).geom.clone());
                first = false;
            }
            pl = pl.must_extend(map.get_l(pair[0].dst).lane_center_pts.clone());
            pl = pl.must_extend(map.get_t(pair[1]).geom.clone());
        }
        pl
    }
}
