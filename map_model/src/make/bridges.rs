use crate::{Road, RoadID};
use abstutil::Timer;
use geom::{Bounds, Distance, FindClosest};

pub fn find_bridges(roads: &mut Vec<Road>, bounds: &Bounds, timer: &mut Timer) {
    let mut closest: FindClosest<RoadID> = FindClosest::new(bounds);
    let mut bridges = Vec::new();
    for r in roads.iter() {
        closest.add(r.id, r.center_pts.points());
        if r.osm_tags.contains_key("bridge") {
            bridges.push(r.id);
        }
    }

    timer.start_iter("find roads underneath bridge", bridges.len());
    for bridge in bridges {
        timer.next();
        let bridge_pts = roads[bridge.0].center_pts.clone();
        for (r, _, _) in closest.all_close_pts(bridge_pts.middle(), Distance::meters(500.0)) {
            if bridge != r && bridge_pts.intersection(&roads[r.0].center_pts).is_some() {
                if roads[r.0].zorder == 0 {
                    roads[r.0].zorder = -1;
                }
            }
        }
    }
}
