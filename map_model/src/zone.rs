use crate::pathfind::cost;
use crate::{
    Intersection, IntersectionID, LaneID, Map, Path, PathConstraints, PathRequest, PathStep,
    Position, RoadID, TurnID,
};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ZoneID(pub usize);

impl fmt::Display for ZoneID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Zone #{}", self.0)
    }
}

// A contiguous set of roads with access restrictions
#[derive(Serialize, Deserialize, Debug)]
pub struct Zone {
    pub id: ZoneID,
    pub members: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
}

impl Zone {
    // If entering, then finds the (last lane outside the zone, first lane inside the zone). If
    // not, the opposite.
    pub(crate) fn find_border(
        &self,
        outside: Position,
        entering: bool,
        constraints: PathConstraints,
        map: &Map,
    ) -> Option<(LaneID, LaneID)> {
        let mut borders: Vec<&Intersection> = self.borders.iter().map(|i| map.get_i(*i)).collect();
        // TODO Use the CH
        let pt = outside.pt(map);
        borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

        for i in borders {
            let src = i
                .get_incoming_lanes(map, constraints)
                .find(|l| self.members.contains(&map.get_l(*l).parent) == !entering)?;
            let dst = i
                .get_outgoing_lanes(map, constraints)
                .into_iter()
                .find(|l| self.members.contains(&map.get_l(*l).parent) == entering)?;
            return Some((src, dst));
        }
        None
    }

    // Run slower Dijkstra's within the interior of a private zone. Don't go outside the borders.
    pub(crate) fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        // Edge type is the Turn, but we don't need it
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
            |(_, _, turn)| cost(map.get_l(turn.src), map.get_t(*turn), req.constraints, map),
            |_| 0,
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
        Some(Path::new(map, steps, req.end.dist_along()))
    }
}
