use std::collections::{HashMap, HashSet};

use aabb_quadtree::QuadTree;
use abstutil::Timer;

use crate::{osm, RawMap};

/// Look for roads that physically overlap, but aren't connected by an intersection. Shrink their
/// width.
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

        let (center, total_width) = road.untrimmed_road_geometry();
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
    let mut shrunk = HashSet::new();
    for (r1, r2) in overlapping {
        timer.next();
        // TODO It'd be better to gradually shrink each road until they stop touching. I got that
        // working in some maps, but it crashes in others (downstream in intersection polygon code)
        // for unknown reasons. Just do the simple thing for now.
        for id in [r1, r2] {
            // Don't shrink any road twice!
            if shrunk.contains(&id) {
                continue;
            }
            shrunk.insert(id);
            // Anything derived from lane_specs_ltr will need to be changed. When we store
            // untrimmed and trimmed center points instead of calculating them dynamically, that'll
            // have to happen here.
            for spec in &mut raw.roads.get_mut(&id).unwrap().lane_specs_ltr {
                spec.width *= 0.5;
            }
        }
    }
}
