use std::collections::BTreeSet;

use crate::{osm, DrivingSide, LaneType, OriginalRoad, RawMap};

/// Find dual carriageways that split very briefly, with no intermediate roads, and collapse them.
pub fn collapse_sausage_links(raw: &mut RawMap) {
    for (id1, id2) in find_sausage_links(raw) {
        fix(raw, id1, id2);
    }
}

fn fix(raw: &mut RawMap, id1: OriginalRoad, id2: OriginalRoad) {
    // We're never modifying intersections, so even if sausage links are clustered together, both
    // roads should always continue to exist as we fix things.
    assert!(raw.roads.contains_key(&id1));
    assert!(raw.roads.contains_key(&id2));

    // Arbitrarily remove the 2nd
    let mut road2 = raw.roads.remove(&id2).unwrap();
    // And modify the 1st
    let road1 = raw.roads.get_mut(&id1).unwrap();

    // Geometry
    //
    // Idea 1) Just make a straight line between the intersections. In OSM, usually the two pieces
    // bend away from the median in some unrealistic way.
    // Idea 2) Try to average the two PolyLines somehow
    road1.osm_center_points = vec![
        road1.osm_center_points[0],
        *road1.osm_center_points.last().unwrap(),
    ];

    // Lanes
    //
    // We need to append road2's lanes onto road1's.
    // - Fixing the direction of the lanes
    // - Handling mistagged or mis-inferred sidewalks
    // - Appending them on the left or the right?
    if raw.config.driving_side == DrivingSide::Right {
        // Assume there's not a sidewalk in the middle of the road
        if road1.lane_specs_ltr[0].lt == LaneType::Sidewalk {
            road1.lane_specs_ltr.remove(0);
        }
        if road2.lane_specs_ltr[0].lt == LaneType::Sidewalk {
            road2.lane_specs_ltr.remove(0);
        }

        for mut lane in road2.lane_specs_ltr {
            lane.dir = lane.dir.opposite();
            road1.lane_specs_ltr.insert(0, lane);
        }
    } else {
        if road1.lane_specs_ltr.last().unwrap().lt == LaneType::Sidewalk {
            road1.lane_specs_ltr.pop().unwrap();
        }
        road2.lane_specs_ltr.reverse();
        if road2.lane_specs_ltr[0].lt == LaneType::Sidewalk {
            road2.lane_specs_ltr.remove(0);
        }

        for mut lane in road2.lane_specs_ltr {
            lane.dir = lane.dir.opposite();
            road1.lane_specs_ltr.push(lane);
        }
    }

    // Tags
    // TODO We shouldn't need to modify road1's tags; lanes_ltr are the source of truth. But...
    // other pieces of code still treat tags as an "original" source of truth. Reverting the road
    // to its original state in the lane editor, for example, will get confused here and only see
    // the original road1.
}

fn find_sausage_links(raw: &RawMap) -> BTreeSet<(OriginalRoad, OriginalRoad)> {
    let mut pairs: BTreeSet<(OriginalRoad, OriginalRoad)> = BTreeSet::new();

    for (id1, road1) in &raw.roads {
        // TODO People often forget to fix the lanes when splitting a dual carriageway, but don't
        // attempt to detect/repair that yet.
        if road1.oneway_for_driving().is_none() {
            continue;
        }
        // Find roads that lead between the two endpoints
        let mut common_roads: BTreeSet<OriginalRoad> = into_set(raw.roads_per_intersection(id1.i1))
            .intersection(&into_set(raw.roads_per_intersection(id1.i2)))
            .cloned()
            .collect();
        // Normally it's just this one road
        assert!(common_roads.remove(id1));
        // If there's many roads between these intersections, something weird is happening; ignore
        // it
        if common_roads.len() == 1 {
            let id2 = common_roads.into_iter().next().unwrap();
            // Ignore if we've already found this match
            if pairs.contains(&(id2, *id1)) {
                continue;
            }

            let road2 = &raw.roads[&id2];
            if road2.oneway_for_driving().is_some()
                && road1.osm_tags.get(osm::NAME) == road2.osm_tags.get(osm::NAME)
            {
                pairs.insert((*id1, id2));
            }
        }
    }

    pairs
}

fn into_set<T: Ord>(list: Vec<T>) -> BTreeSet<T> {
    list.into_iter().collect()
}
