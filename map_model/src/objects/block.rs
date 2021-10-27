use std::collections::HashSet;

use anyhow::Result;

use abstutil::wraparound_get;
use geom::{Polygon, Pt2D, Ring};

use crate::{LaneID, Map, RoadSideID, SideOfRoad};

/// A block is defined by a perimeter that traces along the sides of roads. Inside the perimeter,
/// the block may contain buildings and interior roads. In the simple case, a block represents a
/// single "city block", with no interior roads. It may also cover a "neighborhood", where the
/// perimeter contains some "major" and the interior consists only of "minor" roads.
// TODO Maybe "block" is a misleading term. "Contiguous road trace area"?
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
}

impl Block {
    /// Starting at any lane, snap to the nearest side of that road, then begin tracing a single
    /// block, with no interior roads. This will fail if a map boundary is reached. The results are
    /// unusual when crossing the entrance to a tunnel or bridge.
    pub fn single_block(map: &Map, start: LaneID) -> Result<Block> {
        let perimeter = RoadLoop::single_block(map, start);

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
}
