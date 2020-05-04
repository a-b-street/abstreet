use crate::{IntersectionID, Map, TurnID};
use petgraph::graphmap::UnGraphMap;
use std::collections::{HashMap, HashSet};

// This only applies to VehiclePathfinder; walking through these intersections is nothing special.
// TODO I haven't seen any cases yet with "interior" intersections. Some stuff might break.
pub struct IntersectionCluster {
    pub members: HashSet<IntersectionID>,
    pub uber_turns: Vec<UberTurn>,
}

pub struct UberTurn {
    pub path: Vec<TurnID>,
}

pub fn find(map: &Map) -> Vec<IntersectionCluster> {
    let mut clusters = Vec::new();
    let mut graph: UnGraphMap<IntersectionID, ()> = UnGraphMap::new();
    let mut all_restrictions = Vec::new();
    for from in map.all_roads() {
        for (via, to) in &from.complicated_turn_restrictions {
            // Each of these tells us 2 intersections to group together
            let r = map.get_r(*via);
            graph.add_edge(r.src_i, r.dst_i, ());
            all_restrictions.push((from.id, *via, *to));
        }
    }
    for intersections in petgraph::algo::kosaraju_scc(&graph) {
        let members: HashSet<IntersectionID> = intersections.iter().cloned().collect();

        // Find all entrances and exits through this group of intersections
        let mut entrances = Vec::new();
        let mut exits = HashSet::new();
        for i in &intersections {
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
        let orig_num = uber_turns.len();

        // Filter out the restricted ones!
        // TODO Could be more efficient, but eh
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
            ok
        });

        println!(
            "Cluster {:?} has {} uber-turns ({} filtered out):",
            members,
            uber_turns.len(),
            orig_num - uber_turns.len()
        );
        /*for ut in &uber_turns {
            print!("- {}", ut.path[0].src);
            for t in &ut.path {
                print!("-> {}", t.dst);
            }
            println!("");
        }*/

        clusters.push(IntersectionCluster {
            members,
            uber_turns,
        });
    }

    clusters
}

fn flood(start: TurnID, map: &Map, exits: &HashSet<TurnID>) -> Vec<UberTurn> {
    if exits.contains(&start) {
        return vec![UberTurn { path: vec![start] }];
    }

    let mut results = Vec::new();
    let mut preds: HashMap<TurnID, TurnID> = HashMap::new();
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

fn trace_back(end: TurnID, preds: &HashMap<TurnID, TurnID>) -> Vec<TurnID> {
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
