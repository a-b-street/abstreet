use abstutil::{Counter, Timer};
use geom::HashablePt2D;
use map_model::{osm, raw_data, IntersectionType};
use std::collections::{HashMap, HashSet};

pub fn split_up_roads(
    (mut map, roads, traffic_signals, osm_node_ids): (
        raw_data::Map,
        Vec<raw_data::Road>,
        HashSet<HashablePt2D>,
        HashMap<HashablePt2D, i64>,
    ),
    timer: &mut Timer,
) -> raw_data::Map {
    timer.start("splitting up roads");

    let mut next_intersection_id = 0;

    let mut pt_to_intersection: HashMap<HashablePt2D, raw_data::StableIntersectionID> =
        HashMap::new();
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

    for (pt, id) in &pt_to_intersection {
        map.intersections.insert(
            *id,
            raw_data::Intersection {
                point: pt.to_pt2d(),
                orig_id: raw_data::OriginalIntersection {
                    osm_node_id: osm_node_ids[pt],
                },
                intersection_type: if traffic_signals.contains(pt) {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
                label: None,
                synthetic: false,
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

        for pt in &orig_road.center_points {
            pts.push(*pt);
            if pts.len() == 1 {
                continue;
            }
            if let Some(i2) = pt_to_intersection.get(&pt.to_hashable()) {
                r.i2 = *i2;
                if r.i1 == endpt1 {
                    r.osm_tags
                        .insert(osm::ENDPT_BACK.to_string(), "true".to_string());
                }
                if r.i2 == endpt2 {
                    r.osm_tags
                        .insert(osm::ENDPT_FWD.to_string(), "true".to_string());
                }
                r.orig_id.node1 = osm_node_ids[&pts[0].to_hashable()];
                r.orig_id.node2 = osm_node_ids[&pts.last().unwrap().to_hashable()];
                r.center_points = std::mem::replace(&mut pts, Vec::new());
                // Start a new road
                map.roads
                    .insert(raw_data::StableRoadID(map.roads.len()), r.clone());
                r.osm_tags.remove(osm::ENDPT_FWD);
                r.osm_tags.remove(osm::ENDPT_BACK);
                r.i1 = *i2;
                pts.push(*pt);
            }
        }
        assert!(pts.len() == 1);
    }

    timer.stop("splitting up roads");
    map
}
