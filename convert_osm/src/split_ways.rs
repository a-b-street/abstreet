use crate::extract::OsmExtract;
use abstutil::{Counter, Timer};
use geom::{Distance, HashablePt2D, Pt2D};
use map_model::raw::{OriginalRoad, RawIntersection, RawMap};
use map_model::{osm, IntersectionType};
use std::collections::HashMap;

// Returns amenities and a mapping of all points to split road. (Some internal points on roads are
// removed, so this mapping isn't redundant.)
pub fn split_up_roads(
    map: &mut RawMap,
    mut input: OsmExtract,
    timer: &mut Timer,
) -> (
    Vec<(Pt2D, String, String)>,
    HashMap<HashablePt2D, OriginalRoad>,
) {
    timer.start("splitting up roads");

    let mut pt_to_intersection: HashMap<HashablePt2D, osm::NodeID> = HashMap::new();
    let mut counts_per_pt = Counter::new();
    for (_, r) in &input.roads {
        for (idx, raw_pt) in r.center_points.iter().enumerate() {
            let pt = raw_pt.to_hashable();
            let count = counts_per_pt.inc(pt);

            // All start and endpoints of ways are also intersections.
            if count == 2 || idx == 0 || idx == r.center_points.len() - 1 {
                if !pt_to_intersection.contains_key(&pt) {
                    let id = input.osm_node_ids[&pt];
                    pt_to_intersection.insert(pt, id);
                }
            }
        }
    }

    for (pt, id) in &pt_to_intersection {
        map.intersections.insert(
            *id,
            RawIntersection {
                point: pt.to_pt2d(),
                intersection_type: if input.traffic_signals.remove(pt).is_some() {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
                // Filled out later
                elevation: Distance::ZERO,
            },
        );
    }

    let mut pt_to_road: HashMap<HashablePt2D, OriginalRoad> = HashMap::new();

    // Now actually split up the roads based on the intersections
    timer.start_iter("split roads", input.roads.len());
    for (osm_way_id, orig_road) in &input.roads {
        timer.next();
        let mut r = orig_road.clone();
        let mut pts = Vec::new();
        let endpt1 = pt_to_intersection[&orig_road.center_points[0].to_hashable()];
        let endpt2 = pt_to_intersection[&orig_road.center_points.last().unwrap().to_hashable()];
        let mut i1 = endpt1;

        for pt in &orig_road.center_points {
            pts.push(*pt);
            if pts.len() == 1 {
                continue;
            }
            if let Some(i2) = pt_to_intersection.get(&pt.to_hashable()) {
                if i1 == endpt1 {
                    r.osm_tags
                        .insert(osm::ENDPT_BACK.to_string(), "true".to_string());
                }
                if *i2 == endpt2 {
                    r.osm_tags
                        .insert(osm::ENDPT_FWD.to_string(), "true".to_string());
                }
                let id = OriginalRoad {
                    osm_way_id: *osm_way_id,
                    i1,
                    i2: *i2,
                };
                for pt in &pts {
                    pt_to_road.insert(pt.to_hashable(), id);
                }

                r.center_points = dedupe_angles(std::mem::replace(&mut pts, Vec::new()));
                // Start a new road
                map.roads.insert(id, r.clone());
                r.osm_tags.remove(osm::ENDPT_FWD);
                r.osm_tags.remove(osm::ENDPT_BACK);
                i1 = *i2;
                pts.push(*pt);
            }
        }
        assert!(pts.len() == 1);
    }

    // Resolve simple turn restrictions (via a node)
    let mut restrictions = Vec::new();
    for (restriction, from_osm, via_osm, to_osm) in input.simple_turn_restrictions {
        let roads = map.roads_per_intersection(via_osm);
        // If some of the roads are missing, they were likely filtered out -- usually service
        // roads.
        if let (Some(from), Some(to)) = (
            roads.iter().find(|r| r.osm_way_id == from_osm),
            roads.iter().find(|r| r.osm_way_id == to_osm),
        ) {
            restrictions.push((*from, restriction, *to));
        }
    }
    for (from, rt, to) in restrictions {
        map.roads
            .get_mut(&from)
            .unwrap()
            .turn_restrictions
            .push((rt, to));
    }

    // Resolve complicated turn restrictions (via a way). TODO Only handle via ways immediately
    // connected to both roads, for now
    let mut complicated_restrictions = Vec::new();
    for (rel_osm, from_osm, via_osm, to_osm) in input.complicated_turn_restrictions {
        let via_candidates: Vec<OriginalRoad> = map
            .roads
            .keys()
            .filter(|r| r.osm_way_id == via_osm)
            .cloned()
            .collect();
        if via_candidates.len() != 1 {
            timer.warn(format!(
                "Couldn't resolve turn restriction from way {} to way {} via way {}. Candidate \
                 roads for via: {:?}. See {}",
                from_osm, to_osm, via_osm, via_candidates, rel_osm
            ));
            continue;
        }
        let via = via_candidates[0];

        let maybe_from = map
            .roads_per_intersection(via.i1)
            .into_iter()
            .chain(map.roads_per_intersection(via.i2).into_iter())
            .find(|r| r.osm_way_id == from_osm);
        let maybe_to = map
            .roads_per_intersection(via.i1)
            .into_iter()
            .chain(map.roads_per_intersection(via.i2).into_iter())
            .find(|r| r.osm_way_id == to_osm);
        match (maybe_from, maybe_to) {
            (Some(from), Some(to)) => {
                complicated_restrictions.push((from, via, to));
            }
            _ => {
                timer.warn(format!(
                    "Couldn't resolve turn restriction from {} to {} via {:?}",
                    from_osm, to_osm, via
                ));
            }
        }
    }
    for (from, via, to) in complicated_restrictions {
        map.roads
            .get_mut(&from)
            .unwrap()
            .complicated_turn_restrictions
            .push((via, to));
    }

    timer.start("match traffic signals to intersections");
    // Handle traffic signals tagged on incoming ways and not at intersections
    // (https://wiki.openstreetmap.org/wiki/Tag:highway=traffic%20signals?uselang=en#Tag_all_incoming_ways).
    let mut pt_to_road: HashMap<HashablePt2D, OriginalRoad> = HashMap::new();
    for (id, r) in &map.roads {
        for (idx, pt) in r.center_points.iter().enumerate() {
            if idx != 0 && idx != r.center_points.len() - 1 {
                pt_to_road.insert(pt.to_hashable(), *id);
            }
        }
    }
    for (pt, forwards) in input.traffic_signals {
        if let Some(r) = pt_to_road.get(&pt) {
            // Example: https://www.openstreetmap.org/node/26734224
            if !map.roads[r].osm_tags.is(osm::HIGHWAY, "construction") {
                let i = if forwards { r.i2 } else { r.i1 };
                map.intersections.get_mut(&i).unwrap().intersection_type =
                    IntersectionType::TrafficSignal;
            }
        }
    }
    timer.stop("match traffic signals to intersections");

    timer.stop("splitting up roads");
    (input.amenities, pt_to_road)
}

// TODO Consider doing this in PolyLine::new always. extend() there does this too.
fn dedupe_angles(pts: Vec<Pt2D>) -> Vec<Pt2D> {
    let mut result = Vec::new();
    for (idx, pt) in pts.into_iter().enumerate() {
        let l = result.len();
        if idx == 0 || idx == 1 {
            result.push(pt);
        } else if result[l - 2]
            .angle_to(result[l - 1])
            .approx_eq(result[l - 1].angle_to(pt), 0.1)
        {
            result.pop();
            result.push(pt);
        } else {
            result.push(pt);
        }
    }
    result
}
