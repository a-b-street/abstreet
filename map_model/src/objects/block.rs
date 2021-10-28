use std::collections::{HashMap, HashSet};

use anyhow::Result;

use abstutil::wraparound_get;
use geom::{Polygon, Pt2D, Ring};

use crate::{LaneID, Map, RoadID, RoadSideID, SideOfRoad};

/// A block is defined by a perimeter that traces along the sides of roads. Inside the perimeter,
/// the block may contain buildings and interior roads. In the simple case, a block represents a
/// single "city block", with no interior roads. It may also cover a "neighborhood", where the
/// perimeter contains some "major" and the interior consists only of "minor" roads.
// TODO Maybe "block" is a misleading term. "Contiguous road trace area"?
#[derive(Clone)]
pub struct Block {
    pub perimeter: RoadLoop,
    /// The polygon covers the interior of the block.
    pub polygon: Polygon,
    // TODO Track interior buildings and roads
}

/// A sequence of roads in order, beginning and ending at the same place. No "crossings" -- tracing
/// along this sequence should geometrically yield a simple polygon.
// TODO Handle the map boundary. Sometimes this loop should be broken up by border intersections or
// possibly by water/park areas.
#[derive(Clone)]
pub struct RoadLoop {
    pub roads: Vec<RoadSideID>,
}

impl RoadLoop {
    fn single_block(map: &Map, start: LaneID) -> RoadLoop {
        let mut roads = Vec::new();
        let start_road_side = map.get_l(start).get_nearest_side_of_road(map);
        // We need to track which side of the road we're at, but also which direction we're facing
        let mut current_road_side = start_road_side;
        let mut current_intersection = map.get_l(start).dst_i;
        loop {
            let i = map.get_i(current_intersection);
            let sorted_roads = i.get_road_sides_sorted_by_incoming_angle(map);
            let idx = sorted_roads
                .iter()
                .position(|x| *x == current_road_side)
                .unwrap() as isize;
            // Do we go clockwise or counter-clockwise around the intersection? Well, unless we're
            // at a dead-end, we want to avoid the other side of the same road.
            let mut next = *wraparound_get(&sorted_roads, idx + 1);
            assert_ne!(next, current_road_side);
            if next.id == current_road_side.id {
                next = *wraparound_get(&sorted_roads, idx - 1);
                assert_ne!(next, current_road_side);
                if next.id == current_road_side.id {
                    // We must be at a dead-end
                    assert_eq!(2, sorted_roads.len());
                }
            }
            roads.push(current_road_side);
            current_road_side = next;
            current_intersection = map
                .get_r(current_road_side.id)
                .other_endpt(current_intersection);

            if current_road_side == start_road_side {
                roads.push(start_road_side);
                break;
            }
        }
        assert_eq!(roads[0], *roads.last().unwrap());
        RoadLoop { roads }
    }

    /// Merges two loops using a road in common. Mutates the current loop. Panics if they don't have that road in common.
    /// TODO What if they share many roads?
    pub fn merge(&mut self, mut other: RoadLoop, common_road: RoadID) {
        // TODO Alt algorithm would rotate until common is first or last...
        let idx1 = self
            .roads
            .iter()
            .position(|x| x.id == common_road)
            .unwrap_or_else(|| panic!("First RoadLoop doesn't have {}", common_road));
        let idx2 = other
            .roads
            .iter()
            .position(|x| x.id == common_road)
            .unwrap_or_else(|| panic!("Second RoadLoop doesn't have {}", common_road));

        // The first element is the common road, now an interior
        let last_pieces = self.roads.split_off(idx1);
        let mut middle_pieces = other.roads.split_off(idx2);
        // We repeat the first and last road, but we don't want that for the middle piece
        middle_pieces.pop();

        // TODO just operate on self
        let mut roads = std::mem::take(&mut self.roads);
        roads.extend(middle_pieces.into_iter().skip(1));
        roads.append(&mut other.roads);
        roads.extend(last_pieces.into_iter().skip(1));

        // If the common_road is the first or last, we might wind up not matching here...
        if roads[0] != *roads.last().unwrap() {
            roads.push(roads[0]);
        }

        println!("common was {}. sup {:?}", common_road, roads);
        self.roads = roads;
    }

    /// Find an arbitrary road that two loops have in common.
    pub fn find_common_road(&self, other: &RoadLoop) -> Option<RoadID> {
        let mut roads = HashSet::new();
        for id in self.roads.iter().skip(1) {
            roads.insert(id.id);
        }
        for id in &other.roads {
            if roads.contains(&id.id) {
                return Some(id.id);
            }
        }
        None
    }

    /// Consider the loops as a graph, with adjacency determined by sharing any road in common.
    /// Merge all adjacent loops that the predicate allows. Returns the partitioning of adjacent
    /// loops; all of them should be able to sent through merge_all and wind up with one result...
    pub fn partition_by_predicate<F: Fn(RoadID) -> bool>(
        input: Vec<RoadLoop>,
        predicate: F,
    ) -> Vec<Vec<RoadLoop>> {
        let mut road_to_loops: HashMap<RoadID, Vec<usize>> = HashMap::new();
        for (idx, perimeter) in input.iter().enumerate() {
            for id in &perimeter.roads {
                road_to_loops
                    .entry(id.id)
                    .or_insert_with(Vec::new)
                    .push(idx);
            }
        }

        // Start at one loop, floodfill to adjacent loops, subject to the predicate. Returns the
        // indices of everything in that component.
        let floodfill = |start: usize| -> HashSet<usize> {
            let mut visited = HashSet::new();
            let mut queue = vec![start];
            while !queue.is_empty() {
                let current = queue.pop().unwrap();
                if visited.contains(&current) {
                    continue;
                }
                visited.insert(current);
                for id in &input[current].roads {
                    if predicate(id.id) {
                        queue.extend(road_to_loops[&id.id].clone());
                    }
                }
            }
            visited
        };

        let mut partitions: Vec<HashSet<usize>> = Vec::new();
        let mut finished: HashSet<usize> = HashSet::new();
        for start in 0..input.len() {
            if finished.contains(&start) {
                continue;
            }
            let partition = floodfill(start);
            finished.extend(partition.clone());
            partitions.push(partition);
        }

        // Map the indices back to the actual loops.
        let mut loops: Vec<Option<RoadLoop>> = input.into_iter().map(Some).collect();
        let mut results = Vec::new();
        for indices in partitions {
            let mut partition = Vec::new();
            for idx in indices {
                partition.push(loops[idx].take().unwrap());
            }
            results.push(partition);
        }
        // Sanity check
        for maybe_loop in loops {
            assert!(maybe_loop.is_none());
        }
        results
    }

    pub fn to_block(self, map: &Map) -> Result<Block> {
        Block::from_loop(map, self)
    }
}

impl Block {
    /// Starting at any lane, snap to the nearest side of that road, then begin tracing a single
    /// block, with no interior roads. This will fail if a map boundary is reached. The results are
    /// unusual when crossing the entrance to a tunnel or bridge.
    pub fn single_block(map: &Map, start: LaneID) -> Result<Block> {
        Block::from_loop(map, RoadLoop::single_block(map, start))
    }

    fn from_loop(map: &Map, perimeter: RoadLoop) -> Result<Block> {
        // Trace along the loop and build the polygon
        let mut pts: Vec<Pt2D> = Vec::new();
        for pair in perimeter.roads.windows(2) {
            let lane1 = pair[0].get_outermost_lane(map);
            let lane2 = pair[1].get_outermost_lane(map);
            if lane1.id == lane2.id {
                bail!(
                    "Perimeter road has duplicate adjacent. {:?}",
                    perimeter.roads
                );
            }
            let pl = match pair[0].side {
                SideOfRoad::Right => lane1.lane_center_pts.must_shift_right(lane1.width / 2.0),
                SideOfRoad::Left => lane1.lane_center_pts.must_shift_left(lane1.width / 2.0),
            };
            let keep_lane_orientation = if pair[0].id == pair[1].id {
                // We're doubling back at a dead-end. Always follow the orientation of the lane.
                true
            } else {
                match lane1.common_endpt(lane2) {
                    Some(i) => i == lane1.dst_i,
                    None => {
                        // Two different roads link the same two intersections. I don't think we
                        // can decide the order of points other than seeing which endpoint is
                        // closest to our last point.
                        if let Some(last) = pts.last() {
                            last.dist_to(pl.first_pt()) < last.dist_to(pl.last_pt())
                        } else {
                            // The orientation doesn't matter
                            true
                        }
                    }
                }
            };
            if keep_lane_orientation {
                pts.extend(pl.into_points());
            } else {
                pts.extend(pl.reversed().into_points());
            }
        }
        pts.push(pts[0]);
        pts.dedup();
        let polygon = Ring::new(pts)?.into_polygon();

        Ok(Block { perimeter, polygon })
    }

    /// This calculates all single blocks for the entire map. The resulting list does not cover
    /// roads near the map boundary.
    pub fn find_all_single_blocks(map: &Map) -> Vec<Block> {
        let mut seen = HashSet::new();
        let mut blocks = Vec::new();
        for lane in map.all_lanes() {
            let side = lane.get_nearest_side_of_road(map);
            if seen.contains(&side) {
                continue;
            }
            match Block::single_block(map, lane.id) {
                Ok(block) => {
                    seen.extend(block.perimeter.roads.clone());
                    blocks.push(block);
                }
                Err(err) => {
                    // The logs are quite spammy and not helpful yet, since they're all expected
                    // cases near the map boundary
                    if false {
                        warn!("Failed from {}: {}", lane.id, err);
                    }
                    // Don't try again
                    seen.insert(side);
                }
            }
        }
        blocks
    }

    /// Try to merge all given blocks. If successful, only one block will be returned. Blocks are
    /// never "destroyed" -- if not merged, they'll appear in the results.
    /// TODO This may not handle all possible merges yet, the order is brittle...
    pub fn merge_all(map: &Map, list: Vec<Block>) -> Vec<Block> {
        let mut results: Vec<RoadLoop> = Vec::new();
        let input: Vec<RoadLoop> = list.into_iter().map(|x| x.perimeter).collect();

        // To debug, return after any single change
        let mut debug = false;
        for perimeter in input {
            if debug {
                results.push(perimeter);
                continue;
            }

            let mut partner = None;
            for (idx, adjacent) in results.iter().enumerate() {
                if let Some(r) = perimeter.find_common_road(adjacent) {
                    partner = Some((idx, r));
                    break;
                }
            }

            if let Some((idx, r)) = partner {
                results[idx].merge(perimeter, r);
                debug = true;
            } else {
                results.push(perimeter);
            }
        }
        // TODO Fixpoint...
        // TODO Shouldn't be any new errors, right?
        results
            .into_iter()
            .map(|x| Block::from_loop(map, x).unwrap())
            .collect()
    }
}
