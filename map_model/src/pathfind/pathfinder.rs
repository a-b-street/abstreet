use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;

use crate::pathfind::ch::ContractionHierarchyPathfinder;
use crate::pathfind::walking::{one_step_walking_path, walking_path_to_steps};
use crate::pathfind::{dijkstra, WalkingNode};
use crate::{
    BusRouteID, BusStopID, Intersection, LaneID, Map, Path, PathConstraints, PathRequest, Position,
    TurnID, Zone,
};

/// Most of the time, prefer using the faster contraction hierarchies. But sometimes, callers can
/// explicitly opt into a slower (but preparation-free) pathfinder that just uses Dijkstra's
/// maneuever.
#[derive(Serialize, Deserialize)]
pub enum Pathfinder {
    Dijkstra,
    CH(ContractionHierarchyPathfinder),
}

impl Pathfinder {
    /// Finds a path from a start to an end for a certain type of agent. Handles requests that
    /// start or end inside access-restricted zones.
    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        if req.start.lane() == req.end.lane() && req.constraints == PathConstraints::Pedestrian {
            return Some(one_step_walking_path(&req, map));
        }

        // If we start or end in a private zone, have to stitch together a smaller path with a path
        // through the main map.
        let start_r = map.get_parent(req.start.lane());
        let end_r = map.get_parent(req.end.lane());

        match (start_r.get_zone(map), end_r.get_zone(map)) {
            (Some(z1), Some(z2)) => {
                if z1 == z2 {
                    if !z1
                        .restrictions
                        .allow_through_traffic
                        .contains(req.constraints)
                    {
                        if req.constraints == PathConstraints::Pedestrian {
                            let steps =
                                walking_path_to_steps(z1.pathfind_walking(req.clone(), map)?, map);
                            return Some(Path::new(map, steps, req, Vec::new()));
                        }
                        return z1.pathfind(req, map);
                    }
                } else {
                    // TODO Handle paths going between two different zones
                    return None;
                }
            }
            (Some(zone), None) => {
                if !zone
                    .restrictions
                    .allow_through_traffic
                    .contains(req.constraints)
                {
                    let mut borders: Vec<&Intersection> =
                        zone.borders.iter().map(|i| map.get_i(*i)).collect();
                    // TODO Use the CH to pick the lowest overall cost?
                    let pt = req.end.pt(map);
                    borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

                    for i in borders {
                        if let Some(result) = self.pathfind_from_zone(i, req.clone(), zone, map) {
                            return Some(result);
                        }
                    }
                    return None;
                }
            }
            (None, Some(zone)) => {
                if !zone
                    .restrictions
                    .allow_through_traffic
                    .contains(req.constraints)
                {
                    let mut borders: Vec<&Intersection> =
                        zone.borders.iter().map(|i| map.get_i(*i)).collect();
                    // TODO Use the CH to pick the lowest overall cost?
                    let pt = req.start.pt(map);
                    borders.sort_by_key(|i| pt.dist_to(i.polygon.center()));

                    for i in borders {
                        if let Some(result) = self.pathfind_to_zone(i, req.clone(), zone, map) {
                            return Some(result);
                        }
                    }
                    return None;
                }
            }
            (None, None) => {}
        }

        if req.constraints == PathConstraints::Pedestrian {
            if req.start.lane() == req.end.lane() {
                return Some(one_step_walking_path(&req, map));
            }
            let steps = walking_path_to_steps(self.simple_walking_path(&req, map)?, map);
            return Some(Path::new(map, steps, req, Vec::new()));
        }
        self.simple_pathfind(&req, map)
    }

    pub fn pathfind_avoiding_lanes(
        &self,
        req: PathRequest,
        avoid: BTreeSet<LaneID>,
        map: &Map,
    ) -> Option<Path> {
        dijkstra::pathfind_avoiding_lanes(req, avoid, map)
    }

    // TODO Consider returning the walking-only path in the failure case, to avoid wasting work
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        match self {
            // TODO Implement this
            Pathfinder::Dijkstra => None,
            Pathfinder::CH(ref p) => p.should_use_transit(map, start, end),
        }
    }

    pub fn apply_edits(&mut self, map: &Map, timer: &mut Timer) {
        match self {
            Pathfinder::Dijkstra => {}
            Pathfinder::CH(ref mut p) => p.apply_edits(map, timer),
        }
    }

    // Doesn't handle zones or pedestrians
    fn simple_pathfind(&self, req: &PathRequest, map: &Map) -> Option<Path> {
        match self {
            Pathfinder::Dijkstra => dijkstra::simple_pathfind(req, map),
            Pathfinder::CH(ref p) => p.simple_pathfind(req, map),
        }
    }

    fn simple_walking_path(&self, req: &PathRequest, map: &Map) -> Option<Vec<WalkingNode>> {
        match self {
            Pathfinder::Dijkstra => dijkstra::simple_walking_path(req, map),
            Pathfinder::CH(ref p) => p.simple_walking_path(req, map),
        }
    }

    fn pathfind_from_zone(
        &self,
        i: &Intersection,
        mut req: PathRequest,
        zone: &Zone,
        map: &Map,
    ) -> Option<Path> {
        // Because sidewalks aren't all immediately linked, insist on a (src, dst) combo that
        // are actually connected by a turn.
        let src_choices = i
            .get_incoming_lanes(map, req.constraints)
            .into_iter()
            .filter(|l| zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let dst_choices = i
            .get_outgoing_lanes(map, req.constraints)
            .into_iter()
            .filter(|l| !zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let (src, dst) = {
            let mut result = None;
            'OUTER: for l1 in src_choices {
                for l2 in &dst_choices {
                    if l1 != *l2
                        && map
                            .maybe_get_t(TurnID {
                                parent: i.id,
                                src: l1,
                                dst: *l2,
                            })
                            .is_some()
                    {
                        result = Some((l1, *l2));
                        break 'OUTER;
                    }
                }
            }
            result?
        };

        let interior_req = PathRequest {
            start: req.start,
            end: if map.get_l(src).dst_i == i.id {
                Position::end(src, map)
            } else {
                Position::start(src)
            },
            constraints: req.constraints,
        };
        req.start = if map.get_l(dst).src_i == i.id {
            Position::start(dst)
        } else {
            Position::end(dst, map)
        };

        if let PathConstraints::Pedestrian = req.constraints {
            let mut interior_path = zone.pathfind_walking(interior_req, map)?;
            let main_path = if req.start.lane() == req.end.lane() {
                let mut one_step = vec![
                    WalkingNode::closest(req.start, map),
                    WalkingNode::closest(req.end, map),
                ];
                one_step.dedup();
                one_step
            } else {
                self.simple_walking_path(&req, map)?
            };
            interior_path.extend(main_path);
            let steps = walking_path_to_steps(interior_path, map);
            return Some(Path::new(map, steps, req, Vec::new()));
        }

        let mut interior_path = zone.pathfind(interior_req, map)?;
        let main_path = self.simple_pathfind(&req, map)?;
        interior_path.append(main_path, map);
        Some(interior_path)
    }

    fn pathfind_to_zone(
        &self,
        i: &Intersection,
        mut req: PathRequest,
        zone: &Zone,
        map: &Map,
    ) -> Option<Path> {
        // Because sidewalks aren't all immediately linked, insist on a (src, dst) combo that
        // are actually connected by a turn.
        let src_choices = i
            .get_incoming_lanes(map, req.constraints)
            .into_iter()
            .filter(|l| !zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let dst_choices = i
            .get_outgoing_lanes(map, req.constraints)
            .into_iter()
            .filter(|l| zone.members.contains(&map.get_l(*l).parent))
            .collect::<Vec<_>>();
        let (src, dst) = {
            let mut result = None;
            'OUTER: for l1 in src_choices {
                for l2 in &dst_choices {
                    if l1 != *l2
                        && map
                            .maybe_get_t(TurnID {
                                parent: i.id,
                                src: l1,
                                dst: *l2,
                            })
                            .is_some()
                    {
                        result = Some((l1, *l2));
                        break 'OUTER;
                    }
                }
            }
            result?
        };

        let interior_req = PathRequest {
            start: if map.get_l(dst).src_i == i.id {
                Position::start(dst)
            } else {
                Position::end(dst, map)
            },
            end: req.end,
            constraints: req.constraints,
        };
        let orig_req = req.clone();
        req.end = if map.get_l(src).dst_i == i.id {
            Position::end(src, map)
        } else {
            Position::start(src)
        };

        if let PathConstraints::Pedestrian = req.constraints {
            let interior_path = zone.pathfind_walking(interior_req, map)?;
            let mut main_path = if req.start.lane() == req.end.lane() {
                let mut one_step = vec![
                    WalkingNode::closest(req.start, map),
                    WalkingNode::closest(req.end, map),
                ];
                one_step.dedup();
                one_step
            } else {
                self.simple_walking_path(&req, map)?
            };

            main_path.extend(interior_path);
            let steps = walking_path_to_steps(main_path, map);
            return Some(Path::new(map, steps, orig_req, Vec::new()));
        }

        let interior_path = zone.pathfind(interior_req, map)?;
        let mut main_path = self.simple_pathfind(&req, map)?;
        main_path.append(interior_path, map);
        main_path.orig_req = orig_req;
        Some(main_path)
    }
}
