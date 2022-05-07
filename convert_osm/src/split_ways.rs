use std::collections::{hash_map::Entry, HashMap, HashSet};

use abstutil::{Counter, Tags, Timer};
use geom::{Distance, HashablePt2D, PolyLine, Pt2D};
use raw_map::{
    osm, Amenity, Direction, IntersectionType, OriginalRoad, RawIntersection, RawMap, RawRoad,
};

use crate::extract::OsmExtract;

pub struct Output {
    pub amenities: Vec<(Pt2D, Amenity)>,
    pub crosswalks: HashSet<HashablePt2D>,
    /// A mapping of all points to the split road. Some internal points on roads get removed in
    /// `split_up_roads`, so this mapping isn't redundant.
    pub pt_to_road: HashMap<HashablePt2D, OriginalRoad>,
}

pub fn split_up_roads(map: &mut RawMap, mut input: OsmExtract, timer: &mut Timer) -> Output {
    timer.start("splitting up roads");

    let mut roundabout_centers: HashMap<osm::NodeID, Pt2D> = HashMap::new();
    let mut pt_to_intersection: HashMap<HashablePt2D, osm::NodeID> = HashMap::new();

    input.roads.retain(|(id, pts, tags)| {
        if should_collapse_roundabout(pts, tags) {
            info!("Collapsing tiny roundabout {}", id);
            // Arbitrarily use the first node's ID
            let id = input.osm_node_ids[&pts[0].to_hashable()];
            roundabout_centers.insert(id, Pt2D::center(pts));
            for pt in pts {
                pt_to_intersection.insert(pt.to_hashable(), id);
            }

            false
        } else {
            true
        }
    });

    let mut counts_per_pt = Counter::new();
    for (_, pts, _) in &input.roads {
        for (idx, raw_pt) in pts.iter().enumerate() {
            let pt = raw_pt.to_hashable();
            let count = counts_per_pt.inc(pt);

            // All start and endpoints of ways are also intersections.
            if count == 2 || idx == 0 || idx == pts.len() - 1 {
                if let Entry::Vacant(e) = pt_to_intersection.entry(pt) {
                    let id = input.osm_node_ids[&pt];
                    e.insert(id);
                }
            }
        }
    }

    for (pt, id) in &pt_to_intersection {
        map.intersections.insert(
            *id,
            RawIntersection::new(
                pt.to_pt2d(),
                if input.traffic_signals.remove(pt).is_some() {
                    IntersectionType::TrafficSignal
                } else {
                    IntersectionType::StopSign
                },
            ),
        );
    }

    // Set roundabouts to their center
    for (id, point) in roundabout_centers {
        map.intersections
            .insert(id, RawIntersection::new(point, IntersectionType::StopSign));
    }

    let mut pt_to_road: HashMap<HashablePt2D, OriginalRoad> = HashMap::new();

    // Now actually split up the roads based on the intersections
    timer.start_iter("split roads", input.roads.len());
    for (osm_way_id, orig_pts, orig_tags) in &input.roads {
        timer.next();
        let mut tags = orig_tags.clone();
        let mut pts = Vec::new();
        let endpt1 = pt_to_intersection[&orig_pts[0].to_hashable()];
        let endpt2 = pt_to_intersection[&orig_pts.last().unwrap().to_hashable()];
        let mut i1 = endpt1;

        for pt in orig_pts {
            pts.push(*pt);
            if pts.len() == 1 {
                continue;
            }
            if let Some(i2) = pt_to_intersection.get(&pt.to_hashable()) {
                if i1 == endpt1 {
                    tags.insert(osm::ENDPT_BACK.to_string(), "true".to_string());
                }
                if *i2 == endpt2 {
                    tags.insert(osm::ENDPT_FWD.to_string(), "true".to_string());
                }
                let id = OriginalRoad {
                    osm_way_id: *osm_way_id,
                    i1,
                    i2: *i2,
                };
                // Note we populate this before simplify_linestring, so even if some points are
                // removed, we can still associate them to the road.
                for (idx, pt) in pts.iter().enumerate() {
                    if idx != 0 && idx != pts.len() - 1 {
                        pt_to_road.insert(pt.to_hashable(), id);
                    }
                }

                let osm_center_pts = simplify_linestring(std::mem::take(&mut pts));
                match RawRoad::new(osm_center_pts, tags, &map.config) {
                    Ok(road) => {
                        map.roads.insert(id, road);
                    }
                    Err(err) => {
                        error!("Skipping {id}: {err}");
                        // There may be an orphaned intersection left around; a later
                        // transformation should clean it up
                    }
                }

                // Start a new road
                tags = orig_tags.clone();
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
            warn!(
                "Couldn't resolve turn restriction from way {} to way {} via way {}. Candidate \
                 roads for via: {:?}. See {}",
                from_osm, to_osm, via_osm, via_candidates, rel_osm
            );
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
                warn!(
                    "Couldn't resolve turn restriction from {} to {} via {:?}",
                    from_osm, to_osm, via
                );
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
    for (pt, dir) in input.traffic_signals {
        if let Some(r) = pt_to_road.get(&pt) {
            // Example: https://www.openstreetmap.org/node/26734224
            if !map.roads[r].osm_tags.is(osm::HIGHWAY, "construction") {
                let i = if dir == Direction::Fwd { r.i2 } else { r.i1 };
                map.intersections.get_mut(&i).unwrap().intersection_type =
                    IntersectionType::TrafficSignal;
            }
        }
    }
    timer.stop("match traffic signals to intersections");

    timer.stop("splitting up roads");
    Output {
        amenities: input.amenities,
        crosswalks: input.crosswalks,
        pt_to_road,
    }
}

// TODO Consider doing this in PolyLine::new always. Also in extend() -- it attempts to dedupe
// angles.
fn simplify_linestring(pts: Vec<Pt2D>) -> Vec<Pt2D> {
    // Reduce the number of points along curves. They're wasteful, and when they're too close
    // together, actually break PolyLine shifting:
    // https://github.com/a-b-street/abstreet/issues/833
    //
    // The epsilon is in units of meters; points closer than this will get simplified. 0.1 is too
    // loose -- a curve with too many points was still broken, but 1.0 was too aggressive -- curves
    // got noticeably flattened. At 0.5, some intersetion polygons get a bit worse, but only in
    // places where they were already pretty broken.
    let epsilon = 0.5;
    Pt2D::simplify_rdp(pts, epsilon)
}

/// Many "roundabouts" like https://www.openstreetmap.org/way/427144965 are so tiny that they wind
/// up with ridiculous geometry, cause constant gridlock, and prevent merging adjacent blocks.
///
/// Note https://www.openstreetmap.org/way/394991047 is an example of something that shouldn't get
/// modified. The only distinction, currently, is length -- but I'd love a better definition.
/// Possibly the number of connecting roads.
fn should_collapse_roundabout(pts: &[Pt2D], tags: &Tags) -> bool {
    tags.is_any("junction", vec!["roundabout", "circular"])
        && pts[0] == *pts.last().unwrap()
        && PolyLine::unchecked_new(pts.to_vec()).length() < Distance::meters(50.0)
}
