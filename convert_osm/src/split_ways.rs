use abstutil::Timer;
use geom::{GPSBounds, HashablePt2D, LonLat, Pt2D};
use map_model::{raw_data, IntersectionType};
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn split_up_roads(
    (mut roads, buildings, areas, traffic_signals, turn_restrictions): (
        Vec<raw_data::Road>,
        Vec<raw_data::Building>,
        Vec<raw_data::Area>,
        HashSet<HashablePt2D>,
        BTreeMap<i64, Vec<(String, i64)>>,
    ),
    gps_bounds: GPSBounds,
    timer: &mut Timer,
) -> raw_data::Map {
    timer.start("splitting up roads");

    let mut next_intersection_id = 0;

    // Normally one point to one intersection, but all points on a roundabout map to a single
    // point.
    let mut roundabout_centers: HashMap<raw_data::StableIntersectionID, LonLat> = HashMap::new();
    let mut pt_to_intersection: HashMap<HashablePt2D, raw_data::StableIntersectionID> =
        HashMap::new();

    roads.retain(|r| {
        if r.osm_tags.get("junction") == Some(&"roundabout".to_string()) {
            let id = raw_data::StableIntersectionID(next_intersection_id);
            next_intersection_id += 1;

            roundabout_centers.insert(id, LonLat::center(&r.points));
            for pt in &r.points {
                pt_to_intersection.insert(pt.to_hashable(), id);
            }

            false
        } else {
            true
        }
    });

    // Find normal intersections
    let mut counts_per_pt: HashMap<HashablePt2D, usize> = HashMap::new();
    for r in &roads {
        for (idx, raw_pt) in r.points.iter().enumerate() {
            let pt = raw_pt.to_hashable();
            counts_per_pt.entry(pt).or_insert(0);
            let count = counts_per_pt[&pt] + 1;
            counts_per_pt.insert(pt, count);

            // All start and endpoints of ways are also intersections.
            if count == 2 || idx == 0 || idx == r.points.len() - 1 {
                if !pt_to_intersection.contains_key(&pt) {
                    let id = raw_data::StableIntersectionID(next_intersection_id);
                    next_intersection_id += 1;
                    pt_to_intersection.insert(pt, id);
                }
            }
        }
    }

    let mut map = raw_data::Map::blank();
    map.gps_bounds = gps_bounds;
    map.buildings = buildings;
    map.areas = areas;
    map.turn_restrictions = turn_restrictions;

    // All of the roundabout points will just keep moving the intersection
    for (pt, id) in &pt_to_intersection {
        let point = LonLat::new(pt.x(), pt.y());
        map.intersections.insert(
            *id,
            raw_data::Intersection {
                point: Pt2D::forcibly_from_gps(point, &map.gps_bounds),
                orig_id: raw_data::OriginalIntersection { point },
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
                point: Pt2D::forcibly_from_gps(*pt, &map.gps_bounds),
                orig_id: raw_data::OriginalIntersection { point: *pt },
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
        r.points.clear();
        let endpt1 = pt_to_intersection[&orig_road.points[0].to_hashable()];
        let endpt2 = pt_to_intersection[&orig_road.points.last().unwrap().to_hashable()];
        r.i1 = pt_to_intersection[&orig_road.points[0].to_hashable()];

        for (idx, pt) in orig_road.points.iter().enumerate() {
            r.points.push(pt.clone());
            if r.points.len() > 1 {
                if let Some(i2) = pt_to_intersection.get(&pt.to_hashable()) {
                    if roundabout_centers.contains_key(i2) && idx != orig_road.points.len() - 1 {
                        panic!(
                            "OSM way {} hits a roundabout in the middle of a way. idx {} of length {}",
                            r.osm_way_id,
                            idx,
                            r.points.len()
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
                    // Start a new road
                    map.roads
                        .insert(raw_data::StableRoadID(map.roads.len()), r.clone());
                    r.points.clear();
                    r.osm_tags.remove("abst:endpt_fwd");
                    r.osm_tags.remove("abst:endpt_back");
                    r.i1 = *i2;
                    r.points.push(pt.clone());
                }
            }
        }
        assert!(r.points.len() == 1);
    }

    timer.stop("splitting up roads");
    map
}
