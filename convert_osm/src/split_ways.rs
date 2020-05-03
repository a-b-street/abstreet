use abstutil::{Counter, Timer};
use geom::{Distance, HashablePt2D, Pt2D};
use map_model::raw::{
    OriginalIntersection, OriginalRoad, RawIntersection, RawMap, RawRoad, RestrictionType,
};
use map_model::{osm, IntersectionType};
use std::collections::{HashMap, HashSet};

pub fn split_up_roads(
    (
        mut map,
        roads,
        traffic_signals,
        osm_node_ids,
        simple_turn_restrictions,
        complicated_turn_restrictions,
        amenities,
    ): (
        RawMap,
        Vec<(i64, RawRoad)>,
        HashSet<HashablePt2D>,
        HashMap<HashablePt2D, i64>,
        Vec<(RestrictionType, i64, i64, i64)>,
        Vec<(i64, i64, i64)>,
        Vec<(Pt2D, String, String)>,
    ),
    timer: &mut Timer,
) -> (RawMap, Vec<(Pt2D, String, String)>) {
    timer.start("splitting up roads");

    let mut pt_to_intersection: HashMap<HashablePt2D, OriginalIntersection> = HashMap::new();
    let mut counts_per_pt = Counter::new();
    for (_, r) in &roads {
        for (idx, raw_pt) in r.center_points.iter().enumerate() {
            let pt = raw_pt.to_hashable();
            let count = counts_per_pt.inc(pt);

            // All start and endpoints of ways are also intersections.
            if count == 2 || idx == 0 || idx == r.center_points.len() - 1 {
                if !pt_to_intersection.contains_key(&pt) {
                    let id = OriginalIntersection {
                        osm_node_id: osm_node_ids[&pt],
                    };
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
                intersection_type: if traffic_signals.contains(pt) {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
                // Filled out later
                elevation: Distance::ZERO,
            },
        );
    }

    // Now actually split up the roads based on the intersections
    timer.start_iter("split roads", roads.len());
    for (osm_way_id, orig_road) in &roads {
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
                r.center_points = dedupe_angles(std::mem::replace(&mut pts, Vec::new()));
                // Start a new road
                map.roads.insert(
                    OriginalRoad {
                        osm_way_id: *osm_way_id,
                        i1,
                        i2: *i2,
                    },
                    r.clone(),
                );
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
    for (restriction, from_osm, via_osm, to_osm) in simple_turn_restrictions {
        let roads = map.roads_per_intersection(OriginalIntersection {
            osm_node_id: via_osm,
        });
        match (
            roads.iter().find(|r| r.osm_way_id == from_osm),
            roads.iter().find(|r| r.osm_way_id == to_osm),
        ) {
            (Some(from), Some(to)) => {
                restrictions.push((*from, restriction, *to));
            }
            _ => {
                timer.warn(format!(
                    "Couldn't resolve {:?} from {} to {} via node {}",
                    restriction, from_osm, to_osm, via_osm
                ));
            }
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
    for (from_osm, via_osm, to_osm) in complicated_turn_restrictions {
        let via_candidates: Vec<OriginalRoad> = map
            .roads
            .keys()
            .filter(|r| r.osm_way_id == via_osm)
            .cloned()
            .collect();
        if via_candidates.len() != 1 {
            timer.warn(format!(
                "Couldn't resolve turn restriction from {} to {} via way {}. Candidate roads for \
                 via: {:?}",
                from_osm, to_osm, via_osm, via_candidates
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

    timer.stop("splitting up roads");
    (map, amenities)
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
