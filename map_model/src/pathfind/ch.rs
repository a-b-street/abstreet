use crate::pathfind::driving::VehiclePathfinder;
use crate::pathfind::walking::{
    one_step_walking_path, walking_path_to_steps, SidewalkPathfinder, WalkingNode,
};
use crate::{
    BusRouteID, BusStopID, Intersection, Map, Path, PathConstraints, PathRequest, Position, TurnID,
    Zone,
};
use abstutil::Timer;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ContractionHierarchyPathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    train_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    walking_with_transit_graph: SidewalkPathfinder,
}

impl ContractionHierarchyPathfinder {
    pub fn new(map: &Map, timer: &mut Timer) -> ContractionHierarchyPathfinder {
        timer.start("prepare pathfinding for cars");
        let car_graph = VehiclePathfinder::new(map, PathConstraints::Car, None);
        timer.stop("prepare pathfinding for cars");

        // The edge weights for bikes are so different from the driving graph that reusing the node
        // ordering actually hurts!
        timer.start("prepare pathfinding for bikes");
        let bike_graph = VehiclePathfinder::new(map, PathConstraints::Bike, None);
        timer.stop("prepare pathfinding for bikes");

        timer.start("prepare pathfinding for buses");
        let bus_graph = VehiclePathfinder::new(map, PathConstraints::Bus, Some(&car_graph));
        timer.stop("prepare pathfinding for buses");

        timer.start("prepare pathfinding for trains");
        let train_graph = VehiclePathfinder::new(map, PathConstraints::Train, None);
        timer.stop("prepare pathfinding for trains");

        timer.start("prepare pathfinding for pedestrians");
        let walking_graph = SidewalkPathfinder::new(map, false, &bus_graph, &train_graph);
        timer.stop("prepare pathfinding for pedestrians");

        timer.start("prepare pathfinding for pedestrians using transit");
        let walking_with_transit_graph =
            SidewalkPathfinder::new(map, true, &bus_graph, &train_graph);
        timer.stop("prepare pathfinding for pedestrians using transit");

        ContractionHierarchyPathfinder {
            car_graph,
            bike_graph,
            bus_graph,
            train_graph,
            walking_graph,
            walking_with_transit_graph,
        }
    }

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
                            return Some(Path::new(map, steps, req.end.dist_along(), Vec::new()));
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
        match req.constraints {
            PathConstraints::Pedestrian => {
                let steps = walking_path_to_steps(self.walking_graph.pathfind(&req, map)?, map);
                Some(Path::new(map, steps, req.end.dist_along(), Vec::new()))
            }
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Train => self.train_graph.pathfind(&req, map).map(|(p, _)| p),
        }
    }

    // TODO Alright, reconsider refactoring pieces of this again. :)
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
                self.walking_graph.pathfind(&req, map)?
            };
            interior_path.extend(main_path);
            let steps = walking_path_to_steps(interior_path, map);
            return Some(Path::new(map, steps, req.end.dist_along(), Vec::new()));
        }

        let mut interior_path = zone.pathfind(interior_req, map)?;
        let main_path = match req.constraints {
            PathConstraints::Pedestrian => unreachable!(),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Train => self.train_graph.pathfind(&req, map).map(|(p, _)| p),
        }?;
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
        let orig_end_dist = req.end.dist_along();
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
                self.walking_graph.pathfind(&req, map)?
            };

            main_path.extend(interior_path);
            let steps = walking_path_to_steps(main_path, map);
            return Some(Path::new(map, steps, orig_end_dist, Vec::new()));
        }

        let interior_path = zone.pathfind(interior_req, map)?;
        let mut main_path = match req.constraints {
            PathConstraints::Pedestrian => unreachable!(),
            PathConstraints::Car => self.car_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bike => self.bike_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Bus => self.bus_graph.pathfind(&req, map).map(|(p, _)| p),
            PathConstraints::Train => self.train_graph.pathfind(&req, map).map(|(p, _)| p),
        }?;
        main_path.append(interior_path, map);
        main_path.end_dist = orig_end_dist;
        Some(main_path)
    }

    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        self.walking_with_transit_graph
            .should_use_transit(map, start, end)
    }

    pub fn apply_edits(&mut self, map: &Map, timer: &mut Timer) {
        timer.start("apply edits to car pathfinding");
        self.car_graph.apply_edits(map);
        timer.stop("apply edits to car pathfinding");

        timer.start("apply edits to bike pathfinding");
        self.bike_graph.apply_edits(map);
        timer.stop("apply edits to bike pathfinding");

        timer.start("apply edits to bus pathfinding");
        self.bus_graph.apply_edits(map);
        timer.stop("apply edits to bus pathfinding");

        // Can't edit anything related to trains

        timer.start("apply edits to pedestrian pathfinding");
        self.walking_graph
            .apply_edits(map, &self.bus_graph, &self.train_graph);
        timer.stop("apply edits to pedestrian pathfinding");

        timer.start("apply edits to pedestrian using transit pathfinding");
        self.walking_with_transit_graph
            .apply_edits(map, &self.bus_graph, &self.train_graph);
        timer.stop("apply edits to pedestrian using transit pathfinding");
    }
}
