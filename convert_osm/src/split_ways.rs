use abstutil::{Counter, Timer};
use geom::{HashablePt2D, Pt2D};
use map_model::{raw_data, IntersectionType};
use std::collections::{HashMap, HashSet};

pub fn split_up_roads(
    (mut map, mut roads, traffic_signals): (
        raw_data::Map,
        Vec<raw_data::Road>,
        HashSet<HashablePt2D>,
    ),
    timer: &mut Timer,
) -> raw_data::Map {
    timer.start("splitting up roads");

    let mut next_intersection_id = 0;

    // Normally one point to one intersection, but all points on a roundabout map to a single
    // point.
    let mut roundabout_centers: HashMap<raw_data::StableIntersectionID, Pt2D> = HashMap::new();
    let mut pt_to_intersection: HashMap<HashablePt2D, raw_data::StableIntersectionID> =
        HashMap::new();

    roads.retain(|r| {
        if r.osm_tags.get("junction") == Some(&"roundabout".to_string()) {
            let id = raw_data::StableIntersectionID(next_intersection_id);
            next_intersection_id += 1;

            roundabout_centers.insert(id, Pt2D::center(&r.center_points));
            for pt in &r.center_points {
                pt_to_intersection.insert(pt.to_hashable(), id);
            }

            false
        } else {
            true
        }
    });

    // Find normal intersections
    let mut counts_per_pt = Counter::new();
    for r in &roads {
        for (idx, raw_pt) in r.center_points.iter().enumerate() {
            let pt = raw_pt.to_hashable();
            let count = counts_per_pt.inc(pt);

            // All start and endpoints of ways are also intersections.
            if count == 2 || idx == 0 || idx == r.center_points.len() - 1 {
                if !pt_to_intersection.contains_key(&pt) {
                    let id = raw_data::StableIntersectionID(next_intersection_id);
                    next_intersection_id += 1;
                    pt_to_intersection.insert(pt, id);
                }
            }
        }
    }

    // All of the roundabout points will just keep moving the intersection
    for (pt, id) in &pt_to_intersection {
        let point = pt.to_pt2d();
        map.intersections.insert(
            *id,
            raw_data::Intersection {
                point,
                orig_id: raw_data::OriginalIntersection {
                    point: point.forcibly_to_gps(&map.gps_bounds),
                },
                intersection_type: if traffic_signals.contains(&point.to_hashable()) {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
                label: None,
            },
        );
    }
    // Set roundabouts to their center
    for (id, pt) in &roundabout_centers {
        map.intersections.insert(
            *id,
            raw_data::Intersection {
                point: *pt,
                orig_id: raw_data::OriginalIntersection {
                    point: pt.forcibly_to_gps(&map.gps_bounds),
                },
                intersection_type: if traffic_signals.contains(&pt.to_hashable()) {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
                label: None,
            },
        );
    }

    // Now actually split up the roads based on the intersections
    timer.start_iter("split roads", roads.len());
    for orig_road in &roads {
        timer.next();
        let mut r = orig_road.clone();
        let mut pts = Vec::new();
        let endpt1 = pt_to_intersection[&orig_road.center_points[0].to_hashable()];
        let endpt2 = pt_to_intersection[&orig_road.center_points.last().unwrap().to_hashable()];
        r.i1 = endpt1;

        for (idx, pt) in orig_road.center_points.iter().enumerate() {
            pts.push(*pt);
            if pts.len() == 1 {
                continue;
            }
            if let Some(i2) = pt_to_intersection.get(&pt.to_hashable()) {
                if roundabout_centers.contains_key(i2) && idx != orig_road.center_points.len() - 1 {
                    panic!(
                        "OSM way {} hits a roundabout in the middle of a way. idx {} of length {}",
                        r.osm_way_id,
                        idx,
                        pts.len()
                    );
                }

                r.i2 = *i2;
                if r.i1 == endpt1 {
                    r.osm_tags
                        .insert("abst:endpt_back".to_string(), "true".to_string());
                }
                if r.i2 == endpt2 {
                    r.osm_tags
                        .insert("abst:endpt_fwd".to_string(), "true".to_string());
                }
                r.orig_id.pt1 = pts[0].forcibly_to_gps(&map.gps_bounds);
                r.orig_id.pt2 = pts.last().unwrap().forcibly_to_gps(&map.gps_bounds);
                r.center_points = std::mem::replace(&mut pts, Vec::new());
                // Start a new road
                map.roads
                    .insert(raw_data::StableRoadID(map.roads.len()), r.clone());
                r.osm_tags.remove("abst:endpt_fwd");
                r.osm_tags.remove("abst:endpt_back");
                r.i1 = *i2;
                pts.push(*pt);
            }
        }
        assert!(pts.len() == 1);
    }

    timer.stop("splitting up roads");
    map
}
