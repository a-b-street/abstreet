//! Zones and AccessRestrictions are used to model things like:
//! 1) gated communities, where only trips beginning or ending at a building in the neighborhood may
//!    use any of the private roads
//! 2) Stay Healthy Streets, where most car traffic is banned, except for trips beginning/ending in
//!    the zone
//! 3) Congestion capping, where only so many cars per hour can enter the zone

use std::collections::BTreeSet;

use enumset::EnumSet;
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use crate::pathfind::{driving_cost, walking_cost, WalkingNode};
use crate::{
    IntersectionID, LaneID, Map, Path, PathConstraints, PathRequest, PathStep, RoadID, TurnID,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AccessRestrictions {
    pub allow_through_traffic: EnumSet<PathConstraints>,
    pub cap_vehicles_per_hour: Option<usize>,
}

impl AccessRestrictions {
    pub fn new() -> AccessRestrictions {
        AccessRestrictions {
            allow_through_traffic: EnumSet::all(),
            cap_vehicles_per_hour: None,
        }
    }
}

/// A contiguous set of roads with access restrictions. This is derived from all the map's roads and
/// kept cached for performance.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Zone {
    pub members: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
    pub restrictions: AccessRestrictions,
}

impl Zone {
    pub fn make_all(map: &Map) -> Vec<Zone> {
        let mut queue = Vec::new();
        for r in map.all_roads() {
            if r.is_private() {
                queue.push(r.id);
            }
        }

        let mut zones = Vec::new();
        let mut seen = BTreeSet::new();
        while !queue.is_empty() {
            let start = queue.pop().unwrap();
            if seen.contains(&start) {
                continue;
            }
            let zone = floodfill(map, start);
            seen.extend(zone.members.clone());
            zones.push(zone);
        }

        zones
    }

    /// Run slower Dijkstra's within the interior of a private zone. Don't go outside the borders.
    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        assert_ne!(req.constraints, PathConstraints::Pedestrian);

        let mut graph: DiGraphMap<LaneID, TurnID> = DiGraphMap::new();
        for r in &self.members {
            for l in map.get_r(*r).all_lanes() {
                if req.constraints.can_use(map.get_l(l), map) {
                    for turn in map.get_turns_for(l, req.constraints) {
                        if !self.borders.contains(&turn.id.parent) {
                            graph.add_edge(turn.id.src, turn.id.dst, turn.id);
                        }
                    }
                }
            }
        }

        let (_, path) = petgraph::algo::astar(
            &graph,
            req.start.lane(),
            |l| l == req.end.lane(),
            |(_, _, turn)| {
                driving_cost(map.get_l(turn.src), map.get_t(*turn), req.constraints, map)
            },
            |_| 0.0,
        )?;
        let mut steps = Vec::new();
        for pair in path.windows(2) {
            steps.push(PathStep::Lane(pair[0]));
            // We don't need to look for this turn in the map; we know it exists.
            steps.push(PathStep::Turn(TurnID {
                parent: map.get_l(pair[0]).dst_i,
                src: pair[0],
                dst: pair[1],
            }));
        }
        steps.push(PathStep::Lane(req.end.lane()));
        assert_eq!(steps[0], PathStep::Lane(req.start.lane()));
        Some(Path::new(map, steps, req, Vec::new()))
    }

    // TODO Not happy this works so differently
    pub fn pathfind_walking(&self, req: PathRequest, map: &Map) -> Option<Vec<WalkingNode>> {
        let mut graph: DiGraphMap<WalkingNode, usize> = DiGraphMap::new();
        for r in &self.members {
            for l in map.get_r(*r).all_lanes() {
                let l = map.get_l(l);
                if l.is_walkable() {
                    let cost = walking_cost(l.length());
                    let n1 = WalkingNode::SidewalkEndpoint(l.id, true);
                    let n2 = WalkingNode::SidewalkEndpoint(l.id, false);
                    graph.add_edge(n1, n2, cost);
                    graph.add_edge(n2, n1, cost);

                    for turn in map.get_turns_for(l.id, PathConstraints::Pedestrian) {
                        if self.members.contains(&map.get_l(turn.id.dst).parent) {
                            graph.add_edge(
                                WalkingNode::SidewalkEndpoint(l.id, l.dst_i == turn.id.parent),
                                WalkingNode::SidewalkEndpoint(
                                    turn.id.dst,
                                    map.get_l(turn.id.dst).dst_i == turn.id.parent,
                                ),
                                walking_cost(turn.geom.length()),
                            );
                        }
                    }
                }
            }
        }

        let closest_start = WalkingNode::closest(req.start, map);
        let closest_end = WalkingNode::closest(req.end, map);
        let (_, path) = petgraph::algo::astar(
            &graph,
            closest_start,
            |end| end == closest_end,
            |(_, _, cost)| *cost,
            |_| 0,
        )?;
        Some(path)
    }
}

fn floodfill(map: &Map, start: RoadID) -> Zone {
    let match_constraints = map.get_r(start).access_restrictions.clone();
    let merge_zones = map.get_edits().merge_zones;
    let mut queue = vec![start];
    let mut members = BTreeSet::new();
    let mut borders = BTreeSet::new();
    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        if members.contains(&current) {
            continue;
        }
        members.insert(current);
        for r in map.get_next_roads(current) {
            let r = map.get_r(r);
            if r.access_restrictions == match_constraints && merge_zones {
                queue.push(r.id);
            } else {
                borders.insert(map.get_r(current).common_endpt(r));
            }
        }
    }
    assert!(!members.is_empty());
    assert!(!borders.is_empty());
    Zone {
        members,
        borders,
        restrictions: match_constraints,
    }
}
