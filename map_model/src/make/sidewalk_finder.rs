use crate::{Lane, LaneID, Position};
use abstutil::Timer;
use geom::{Bounds, Distance, FindClosest, HashablePt2D};
use std::collections::{HashMap, HashSet};

// If the result doesn't contain a requested point, then there was no matching sidewalk close
// enough.
pub fn find_sidewalk_points(
    bounds: &Bounds,
    pts: HashSet<HashablePt2D>,
    lanes: &Vec<Lane>,
    max_dist_away: Distance,
    timer: &mut Timer,
) -> HashMap<HashablePt2D, Position> {
    if pts.is_empty() {
        return HashMap::new();
    }

    let mut closest: FindClosest<LaneID> = FindClosest::new(bounds);
    timer.start_iter("index lanes", lanes.len());
    for l in lanes {
        timer.next();
        if l.is_sidewalk() {
            closest.add(l.id, l.lane_center_pts.points());
        }
    }

    // For each point, find the closest point to any sidewalk, using the quadtree to prune the
    // search.
    let mut results: HashMap<HashablePt2D, Position> = HashMap::new();
    timer.start_iter("find closest sidewalk point", pts.len());
    for query_pt in pts {
        timer.next();
        if let Some((sidewalk, sidewalk_pt)) = closest.closest_pt(query_pt.to_pt2d(), max_dist_away)
        {
            if let Some(dist_along) = lanes[sidewalk.0].dist_along_of_point(sidewalk_pt) {
                results.insert(query_pt, Position::new(sidewalk, dist_along));
            } else {
                panic!("{} isn't on {} according to dist_along_of_point, even though closest_point thinks it is.\n{}", sidewalk_pt, sidewalk, lanes[sidewalk.0].lane_center_pts);
            }
        }
    }
    results
}
