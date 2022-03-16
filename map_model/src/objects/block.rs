use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::wraparound_get;
use geom::{Polygon, Pt2D, Ring};

use crate::{CommonEndpoint, Direction, LaneID, Map, RoadID, RoadSideID, SideOfRoad};

/// A block is defined by a perimeter that traces along the sides of roads. Inside the perimeter,
/// the block may contain buildings and interior roads. In the simple case, a block represents a
/// single "city block", with no interior roads. It may also cover a "neighborhood", where the
/// perimeter contains some "major" and the interior consists only of "minor" roads.
// TODO Maybe "block" is a misleading term. "Contiguous road trace area"?
#[derive(Clone, Serialize, Deserialize)]
pub struct Block {
    pub perimeter: Perimeter,
    /// The polygon covers the interior of the block.
    pub polygon: Polygon,
}

/// A sequence of roads in order, beginning and ending at the same place. No "crossings" -- tracing
/// along this sequence should geometrically yield a simple polygon.
// TODO Handle the map boundary. Sometimes this perimeter should be broken up by border
// intersections or possibly by water/park areas.
#[derive(Clone, Serialize, Deserialize)]
pub struct Perimeter {
    pub roads: Vec<RoadSideID>,
    /// These roads exist entirely within the perimeter
    pub interior: BTreeSet<RoadID>,
}

impl Perimeter {
    /// Starting at any lane, snap to the nearest side of that road, then begin tracing a single
    /// block, with no interior roads. This will fail if a map boundary is reached. The results are
    /// unusual when crossing the entrance to a tunnel or bridge, and so `skip` is used to avoid
    /// tracing there.
    pub fn single_block(map: &Map, start: LaneID, skip: &HashSet<RoadID>) -> Result<Perimeter> {
        let mut roads = Vec::new();
        let start_road_side = map.get_l(start).get_nearest_side_of_road(map);

        if skip.contains(&start_road_side.road) {
            bail!("Started on a road we shouldn't trace");
        }

        // We need to track which side of the road we're at, but also which direction we're facing
        let mut current_road_side = start_road_side;
        let mut current_intersection = map.get_l(start).dst_i;
        loop {
            let i = map.get_i(current_intersection);
            if i.is_border() {
                bail!("hit the map boundary");
            }
            let mut sorted_roads = i.get_road_sides_sorted_by_incoming_angle(map);
            sorted_roads.retain(|id| !skip.contains(&id.road));
            let idx = sorted_roads
                .iter()
                .position(|x| *x == current_road_side)
                .unwrap() as isize;
            // Do we go clockwise or counter-clockwise around the intersection? Well, unless we're
            // at a dead-end, we want to avoid the other side of the same road.
            let mut next = *wraparound_get(&sorted_roads, idx + 1);
            assert_ne!(next, current_road_side);
            if next.road == current_road_side.road {
                next = *wraparound_get(&sorted_roads, idx - 1);
                assert_ne!(next, current_road_side);
                if next.road == current_road_side.road {
                    // We must be at a dead-end
                    assert_eq!(2, sorted_roads.len());
                }
            }
            roads.push(current_road_side);
            current_road_side = next;
            current_intersection = map
                .get_r(current_road_side.road)
                .other_endpt(current_intersection);

            if current_road_side == start_road_side {
                roads.push(start_road_side);
                break;
            }
        }
        assert_eq!(roads[0], *roads.last().unwrap());
        Ok(Perimeter {
            roads,
            interior: BTreeSet::new(),
        })
    }

    /// This calculates all single block perimeters for the entire map. The resulting list does not
    /// cover roads near the map boundary.
    pub fn find_all_single_blocks(map: &Map) -> Vec<Perimeter> {
        let skip = Perimeter::find_roads_to_skip_tracing(map);

        let mut seen = HashSet::new();
        let mut perimeters = Vec::new();
        for lane in map.all_lanes() {
            let side = lane.get_nearest_side_of_road(map);
            if seen.contains(&side) {
                continue;
            }
            match Perimeter::single_block(map, lane.id, &skip) {
                Ok(perimeter) => {
                    seen.extend(perimeter.roads.clone());
                    perimeters.push(perimeter);
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
        perimeters
    }

    /// Trying to form blocks near railways or cycleways that involve bridges/tunnels often causes
    /// overlapping geometry or blocks that're way too large. These are extremely imperfect
    /// heuristics to avoid the worst problems.
    pub fn find_roads_to_skip_tracing(map: &Map) -> HashSet<RoadID> {
        let mut skip = HashSet::new();
        for r in map.all_roads() {
            if r.is_light_rail() {
                skip.insert(r.id);
            } else if r.is_cycleway() && r.zorder != 0 {
                skip.insert(r.id);
            }
        }
        skip
    }

    /// A perimeter has the first and last road matching up, but that's confusing to
    /// work with. Temporarily undo that.
    fn undo_invariant(&mut self) {
        assert_eq!(Some(self.roads[0]), self.roads.pop());
    }

    /// Restore the first=last invariant. Methods may temporarily break this, but must restore it
    /// before returning.
    fn restore_invariant(&mut self) {
        self.roads.push(self.roads[0]);
    }

    /// Try to merge two blocks. Returns true if this is successful, which will only be when the
    /// blocks are adjacent, but the merge wouldn't create an interior "hole".
    ///
    /// Note this may modify both perimeters and still return `false`. The modification is just to
    /// rotate the order of the road loop; this doesn't logically change the perimeter.
    ///
    /// TODO Due to https://github.com/a-b-street/abstreet/issues/841, it seems like rotation
    /// sometimes breaks `to_block`, so for now, always revert to the original upon failure.
    // TODO Would it be cleaner to return a Result here and always restore the invariant?
    fn try_to_merge(
        &mut self,
        map: &Map,
        other: &mut Perimeter,
        debug_failures: bool,
        use_expensive_blockfinding: bool,
    ) -> bool {
        let orig_self = self.clone();
        let orig_other = other.clone();

        self.undo_invariant();
        other.undo_invariant();

        // Calculate common roads
        let roads1: HashSet<RoadID> = self.roads.iter().map(|id| id.road).collect();
        let roads2: HashSet<RoadID> = other.roads.iter().map(|id| id.road).collect();
        let common: HashSet<RoadID> = roads1.intersection(&roads2).cloned().collect();
        if common.is_empty() {
            if debug_failures {
                warn!("No common roads");
            }
            *self = orig_self;
            *other = orig_other;
            return false;
        }

        // "Rotate" the order of roads, so that all of the overlapping roads are at the end of the
        // list. If the entire perimeter is surrounded by the other, then no rotation needed.
        if self.roads.len() != common.len() {
            while common.contains(&self.roads[0].road)
                || !common.contains(&self.roads.last().unwrap().road)
            {
                self.roads.rotate_left(1);
            }
        }
        // Same thing with the other
        if other.roads.len() != common.len() {
            while common.contains(&other.roads[0].road)
                || !common.contains(&other.roads.last().unwrap().road)
            {
                other.roads.rotate_left(1);
            }
        }

        if debug_failures {
            println!("\nCommon: {:?}\n{:?}\n{:?}", common, self, other);
        }

        if self.reverse_to_fix_winding_order(map, other) {
            // Revert, reverse one, and try again. This should never recurse.
            self.restore_invariant();
            other.restore_invariant();
            self.roads.reverse();
            return self.try_to_merge(map, other, debug_failures, use_expensive_blockfinding);
        }

        // Check if all of the common roads are at the end of each perimeter,
        // so we can "blindly" do this snipping. If this isn't true, then the overlapping portions
        // are split by non-overlapping roads. This happens when merging the two blocks would
        // result in a "hole."
        let mut ok = true;
        for id in self.roads.iter().rev().take(common.len()) {
            if !common.contains(&id.road) {
                if debug_failures {
                    warn!(
                        "The common roads on the first aren't consecutive, near {:?}",
                        id
                    );
                }
                ok = false;
                break;
            }
        }
        for id in other.roads.iter().rev().take(common.len()) {
            if !common.contains(&id.road) {
                if debug_failures {
                    warn!(
                        "The common roads on the second aren't consecutive, near {:?}",
                        id
                    );
                }
                ok = false;
                break;
            }
        }
        if !ok {
            *self = orig_self;
            *other = orig_other;
            return false;
        }

        // Very straightforward snipping now
        for _ in 0..common.len() {
            self.roads.pop().unwrap();
            other.roads.pop().unwrap();
        }

        // This order assumes everything is clockwise to start with.
        self.roads.append(&mut other.roads);

        // TODO This case was introduced with find_roads_to_skip_tracing. Not sure why.
        if self.roads.is_empty() {
            if debug_failures {
                warn!("Two perimeters had every road in common: {:?}", common);
            }
            *self = orig_self;
            *other = orig_other;
            return false;
        }

        self.interior.extend(common);
        self.interior.append(&mut other.interior);

        // Restore the first=last invariant
        self.restore_invariant();

        // Make sure we didn't wind up with any internal dead-ends
        self.collapse_deadends();

        // TODO Something in this method is buggy and produces invalid merges.
        // https://github.com/a-b-street/abstreet/issues/841
        // First try a lightweight detection for problems. If the caller detects the net result is
        // invalid, they'll override this flag and try again.
        let err = if use_expensive_blockfinding {
            self.clone().to_block(map).err()
        } else {
            self.check_continuity(map).err()
        };
        if let Some(err) = err {
            debug!(
                "A merged perimeter couldn't be blockified: {}. {:?}",
                err, self
            );
            *self = orig_self;
            *other = orig_other;
            return false;
        }

        true
    }

    fn check_continuity(&self, map: &Map) -> Result<()> {
        for pair in self.roads.windows(2) {
            let r1 = map.get_r(pair[0].road);
            let r2 = map.get_r(pair[1].road);
            if r1.common_endpoint(r2) == CommonEndpoint::None {
                bail!("Part of the perimeter goes from {:?} to {:?}, but they don't share a common endpoint", pair[0], pair[1]);
            }
        }
        Ok(())
    }

    /// Should we reverse one perimeter to match the winding order?
    ///
    /// This is only meant to be called in the middle of try_to_merge. It assumes both perimeters
    /// have already been rotated so the common roads are at the end. The invariant of first=last
    /// is not true.
    fn reverse_to_fix_winding_order(&self, map: &Map, other: &Perimeter) -> bool {
        // Using geometry to determine winding order is brittle. Look for any common road, and see
        // where it points.
        let common_example = self.roads.last().unwrap().road;
        let last_common_for_self = match map
            .get_r(common_example)
            .common_endpoint(map.get_r(wraparound_get(&self.roads, self.roads.len() as isize).road))
        {
            CommonEndpoint::One(i) => i,
            CommonEndpoint::Both => {
                // If the common road is a loop on the intersection, then this perimeter must be of
                // length 2 (or 3 with the invariant), and reversing it is meaningless.
                return false;
            }
            CommonEndpoint::None => unreachable!(),
        };

        // Find the same road in the other perimeter
        let other_idx = other
            .roads
            .iter()
            .position(|x| x.road == common_example)
            .unwrap() as isize;
        let last_common_for_other = match map
            .get_r(common_example)
            .common_endpoint(map.get_r(wraparound_get(&other.roads, other_idx + 1).road))
        {
            CommonEndpoint::One(i) => i,
            CommonEndpoint::Both => {
                return false;
            }
            CommonEndpoint::None => unreachable!(),
        };
        last_common_for_self == last_common_for_other
    }

    /// Try to merge all given perimeters. If successful, only one perimeter will be returned.
    /// Perimeters are never "destroyed" -- if not merged, they'll appear in the results. If
    /// `stepwise_debug` is true, returns after performing just one merge.
    pub fn merge_all(
        map: &Map,
        mut input: Vec<Perimeter>,
        stepwise_debug: bool,
        use_expensive_blockfinding: bool,
    ) -> Vec<Perimeter> {
        // Internal dead-ends break merging, so first collapse of those. Do this before even
        // looking for neighbors, since find_common_roads doesn't understand dead-ends.
        for p in &mut input {
            p.collapse_deadends();
        }

        loop {
            let mut debug = false;
            let mut results: Vec<Perimeter> = Vec::new();
            let num_input = input.len();
            'INPUT: for mut perimeter in input {
                if debug {
                    results.push(perimeter);
                    continue;
                }

                for other in &mut results {
                    if other.try_to_merge(
                        map,
                        &mut perimeter,
                        stepwise_debug,
                        use_expensive_blockfinding,
                    ) {
                        // To debug, return after any single change
                        debug = stepwise_debug;
                        continue 'INPUT;
                    }
                }

                // No match
                results.push(perimeter);
            }

            // Should we try merging again?
            if results.len() > 1 && results.len() < num_input && !stepwise_debug {
                input = results;
                continue;
            }
            return results;
        }
    }

    /// If the perimeter follows any dead-end roads, "collapse" them and instead make the perimeter
    /// contain the dead-end.
    pub fn collapse_deadends(&mut self) {
        let orig = self.clone();
        self.undo_invariant();

        // TODO Workaround https://github.com/a-b-street/abstreet/issues/834. If this is a loop
        // around a disconnected fragment of road, don't touch it
        if self.roads.len() == 2 && self.roads[0].road == self.roads[1].road {
            self.restore_invariant();
            return;
        }

        // If the dead-end straddles the loop, it's confusing. Just rotate until that's not true.
        while self.roads[0].road == self.roads.last().unwrap().road {
            self.roads.rotate_left(1);
        }

        // TODO This won't handle a deadend that's more than 1 segment long
        let mut roads: Vec<RoadSideID> = Vec::new();
        for id in self.roads.drain(..) {
            if Some(id.road) == roads.last().map(|id| id.road) {
                roads.pop();
                self.interior.insert(id.road);
            } else {
                roads.push(id);
            }
        }

        self.roads = roads;
        if self.roads.is_empty() {
            // TODO This case was introduced with find_roads_to_skip_tracing. Not sure why.
            *self = orig;
            return;
        }
        self.restore_invariant();
    }

    /// Consider the perimeters as a graph, with adjacency determined by sharing any road in common.
    /// Partition adjacent perimeters, subject to the predicate. Each partition should produce a
    /// single result with `merge_all`.
    pub fn partition_by_predicate<F: Fn(RoadID) -> bool>(
        input: Vec<Perimeter>,
        predicate: F,
    ) -> Vec<Vec<Perimeter>> {
        let mut road_to_perimeters: HashMap<RoadID, Vec<usize>> = HashMap::new();
        for (idx, perimeter) in input.iter().enumerate() {
            for id in &perimeter.roads {
                road_to_perimeters
                    .entry(id.road)
                    .or_insert_with(Vec::new)
                    .push(idx);
            }
        }

        // Start at one perimeter, floodfill to adjacent perimeters, subject to the predicate.
        // Returns the indices of everything in that component.
        let floodfill = |start: usize| -> BTreeSet<usize> {
            let mut visited = BTreeSet::new();
            let mut queue = vec![start];
            while !queue.is_empty() {
                let current = queue.pop().unwrap();
                if visited.contains(&current) {
                    continue;
                }
                visited.insert(current);
                for id in &input[current].roads {
                    if predicate(id.road) {
                        queue.extend(road_to_perimeters[&id.road].clone());
                    }
                }
            }
            visited
        };

        let mut partitions: Vec<BTreeSet<usize>> = Vec::new();
        let mut finished: HashSet<usize> = HashSet::new();
        for start in 0..input.len() {
            if finished.contains(&start) {
                continue;
            }
            let partition = floodfill(start);
            finished.extend(partition.clone());
            partitions.push(partition);
        }

        // Map the indices back to the actual perimeters.
        let mut perimeters: Vec<Option<Perimeter>> = input.into_iter().map(Some).collect();
        let mut results = Vec::new();
        for indices in partitions {
            let mut partition = Vec::new();
            for idx in indices {
                partition.push(perimeters[idx].take().unwrap());
            }
            results.push(partition);
        }
        // Sanity check
        for maybe_perimeter in perimeters {
            assert!(maybe_perimeter.is_none());
        }
        results
    }

    /// Assign each perimeter one of `num_colors`, such that no two adjacent perimeters share the
    /// same color. May fail. The resulting colors are expressed as `[0, num_colors)`.
    pub fn calculate_coloring(input: &[Perimeter], num_colors: usize) -> Option<Vec<usize>> {
        let mut road_to_perimeters: HashMap<RoadID, Vec<usize>> = HashMap::new();
        for (idx, perimeter) in input.iter().enumerate() {
            for id in &perimeter.roads {
                road_to_perimeters
                    .entry(id.road)
                    .or_insert_with(Vec::new)
                    .push(idx);
            }
        }

        // Greedily fill out a color for each perimeter, in the same order as the input
        let mut assigned_colors = Vec::new();
        for (this_idx, perimeter) in input.iter().enumerate() {
            let mut available_colors: Vec<bool> =
                std::iter::repeat(true).take(num_colors).collect();
            // Find all neighbors
            for id in &perimeter.roads {
                for other_idx in &road_to_perimeters[&id.road] {
                    // We assign colors in order, so any neighbor index smaller than us has been
                    // chosen
                    if *other_idx < this_idx {
                        available_colors[assigned_colors[*other_idx]] = false;
                    }
                }
            }
            if let Some(color) = available_colors.iter().position(|x| *x) {
                assigned_colors.push(color);
            } else {
                // Too few colors?
                return None;
            }
        }
        Some(assigned_colors)
    }

    pub fn to_block(self, map: &Map) -> Result<Block> {
        // Trace along the perimeter and build the polygon
        let mut pts: Vec<Pt2D> = Vec::new();
        let mut first_intersection = None;
        for pair in self.roads.windows(2) {
            let lane1 = pair[0].get_outermost_lane(map);
            let road1 = map.get_parent(lane1.id);
            let lane2 = pair[1].get_outermost_lane(map);
            // If lane1 and lane2 are the same, then it just means we found a dead-end road with
            // exactly one lane, which is usually a footway or cycleway that legitimately is a
            // dead-end, or connects to some other road we didn't import. We'll just trace around
            // it like a normal dead-end road.
            let mut pl = match pair[0].side {
                SideOfRoad::Right => road1.center_pts.must_shift_right(road1.get_half_width()),
                SideOfRoad::Left => road1.center_pts.must_shift_left(road1.get_half_width()),
            };
            if lane1.dir == Direction::Back {
                pl = pl.reversed();
            }
            let keep_lane_orientation = if pair[0].road == pair[1].road {
                // We're doubling back at a dead-end. Always follow the orientation of the lane.
                true
            } else {
                match lane1.common_endpoint(lane2) {
                    CommonEndpoint::One(i) => i == lane1.dst_i,
                    CommonEndpoint::Both => {
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
                    CommonEndpoint::None => bail!(
                        "{} and {} don't share a common endpoint",
                        lane1.id,
                        lane2.id
                    ),
                }
            };
            if !keep_lane_orientation {
                pl = pl.reversed();
            }

            // Before we add this road's points, try to trace along the polygon's boundary. Usually
            // this has no effect (we'll dedupe points), but sometimes there's an extra curve.
            //
            // Note this logic is similar to how we find SharedSidewalkCorners. Don't rely on that
            // existing, since the outermost lane mightn't be a sidewalk.
            //
            // If the ring.doubles_back(), don't bother. If we tried to trace the boundary, it
            // usually breaks the final Ring we produce. Better to skip bad intersection polygons
            // and still produce a reasonable looking block.
            let prev_i = if keep_lane_orientation {
                lane1.src_i
            } else {
                lane1.dst_i
            };
            if first_intersection.is_none() {
                first_intersection = Some(prev_i);
            }
            if let Some(last_pt) = pts.last() {
                let prev_i = map.get_i(prev_i);
                if let Some(ring) = prev_i.polygon.get_outer_ring() {
                    if !ring.doubles_back() {
                        // At dead-ends, trace around the intersection on the longer side
                        let longer = prev_i.is_deadend();
                        if let Some(slice) = ring.get_slice_between(*last_pt, pl.first_pt(), longer)
                        {
                            pts.extend(slice.into_points());
                        }
                    }
                }
            }

            pts.extend(pl.into_points());
        }
        // Do the intersection boundary tracing for the last piece. We didn't know enough to do it
        // the first time.
        let first_intersection = map.get_i(first_intersection.unwrap());
        if let Some(ring) = first_intersection.polygon.get_outer_ring() {
            if !ring.doubles_back() {
                let longer = first_intersection.is_deadend();
                if let Some(slice) = ring.get_slice_between(*pts.last().unwrap(), pts[0], longer) {
                    pts.extend(slice.into_points());
                }
            }
        }
        pts.push(pts[0]);
        pts.dedup();
        let polygon = Ring::new(pts)?.into_polygon();
        // TODO To debug anyway, can use buggy_new, but there's pretty much always a root problem
        // in the map geometry that should be properly fixed.
        //let polygon = Polygon::buggy_new(pts);

        Ok(Block {
            perimeter: self,
            polygon,
        })
    }

    /// Does this perimeter completely enclose the other?
    pub fn contains(&self, other: &Perimeter) -> bool {
        other
            .roads
            .iter()
            .all(|id| self.interior.contains(&id.road) || self.roads.contains(id))
    }
}

impl fmt::Debug for Perimeter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Perimeter:")?;
        for id in &self.roads {
            writeln!(f, "- {:?} of {}", id.side, id.road)?;
        }
        Ok(())
    }
}
