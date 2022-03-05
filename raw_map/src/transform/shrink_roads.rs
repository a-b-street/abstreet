use std::collections::HashMap;

use aabb_quadtree::QuadTree;
use abstutil::Timer;

use crate::{osm, RawMap};

pub fn shrink(raw: &mut RawMap, timer: &mut Timer) {
    let mut road_centers = HashMap::new();
    let mut road_polygons = HashMap::new();
    let mut overlapping = Vec::new();
    let mut quadtree = QuadTree::default(raw.gps_bounds.to_bounds().as_bbox());
    timer.start_iter("find overlapping roads", raw.roads.len());
    for (id, road) in &raw.roads {
        timer.next();
        if road.is_light_rail() {
            continue;
        }
        // Only attempt this fix for dual carriageways
        if !road.is_oneway() {
            continue;
        }

        let (center, total_width) = match raw.untrimmed_road_geometry(*id) {
            Ok((center, total_width)) => (center, total_width),
            Err(err) => {
                // Crashing in Lisbon because of https://www.openstreetmap.org/node/5754625281 and
                // https://www.openstreetmap.org/node/5754625989
                error!("Not trying to shrink roads near {}", err);
                continue;
            }
        };
        let polygon = center.make_polygons(total_width);

        // Any conflicts with existing?
        for (other_id, _, _) in quadtree.query(polygon.get_bounds().as_bbox()) {
            // Only dual carriageways
            if road.osm_tags.get(osm::NAME) != raw.roads[other_id].osm_tags.get(osm::NAME) {
                continue;
            }
            if !id.has_common_endpoint(*other_id) && polygon.intersects(&road_polygons[other_id]) {
                // If the polylines don't overlap, then it's probably just a bridge/tunnel
                if center.intersection(&road_centers[other_id]).is_none() {
                    overlapping.push((*id, *other_id));
                }
            }
        }

        quadtree.insert_with_box(*id, polygon.get_bounds().as_bbox());
        road_centers.insert(*id, center);
        road_polygons.insert(*id, polygon);
    }

    timer.start_iter("shrink overlapping roads", overlapping.len());
    for (r1, r2) in overlapping {
        timer.next();
        // TODO It'd be better to gradually shrink each road until they stop touching. I got that
        // working in some maps, but it crashes in others (downstream in intersection polygon code)
        // for unknown reasons. Just do the simple thing for now.
        raw.roads.get_mut(&r1).unwrap().scale_width = 0.5;
        raw.roads.get_mut(&r2).unwrap().scale_width = 0.5;
    }
}
