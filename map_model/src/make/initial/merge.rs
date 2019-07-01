use crate::make::initial::{geometry, InitialMap};
use crate::raw_data::{StableIntersectionID, StableRoadID};
use crate::IntersectionType;
use abstutil::Timer;
use geom::Distance;
use std::collections::HashSet;

pub fn short_roads(map: &mut InitialMap, timer: &mut Timer) {
    if false {
        let mut look_at: HashSet<StableIntersectionID> = HashSet::new();
        let orig_count = map.roads.len();

        // Every time we change a road, other roads we might've already processed could shorten, so
        // we have to redo everything. Note that order of merging doesn't SEEM to matter much...
        // tried tackling the shortest roads first, no effect.
        loop {
            if let Some(r) = map
                .roads
                .values()
                .find(|r| r.trimmed_center_pts.length() < Distance::meters(5.0))
            {
                let id = r.id;
                look_at.insert(merge(map, id, timer));
            } else {
                break;
            }
        }

        timer.note(format!(
            "Deleted {} tiny roads",
            orig_count - map.roads.len()
        ));
        for id in look_at {
            if map.intersections.contains_key(&id) {
                timer.note(format!("Check for merged roads near {}", id));
            }
        }
    }
}

// Returns the retained intersection.
pub fn merge(
    map: &mut InitialMap,
    merge_road: StableRoadID,
    timer: &mut Timer,
) -> StableIntersectionID {
    // Arbitrarily kill off the first intersection and keep the second one.
    let (delete_i, keep_i) = {
        let r = &map.roads[&merge_road];
        timer.note(format!(
            "Deleting {}, which has original length {} and trimmed length {}",
            merge_road,
            r.original_center_pts.length(),
            r.trimmed_center_pts.length()
        ));

        (r.src_i, r.dst_i)
    };
    map.roads.remove(&merge_road);
    let deleted_intersection_type = map
        .intersections
        .remove(&delete_i)
        .unwrap()
        .intersection_type;
    {
        let mut i = map.intersections.get_mut(&keep_i).unwrap();
        i.roads.remove(&merge_road);
        if deleted_intersection_type == IntersectionType::TrafficSignal
            && i.intersection_type == IntersectionType::StopSign
        {
            i.intersection_type = deleted_intersection_type;
        }
    }

    let mut new_loops: Vec<StableRoadID> = Vec::new();
    for r in map.roads.values_mut() {
        if r.src_i == delete_i {
            r.src_i = keep_i;
            map.intersections
                .get_mut(&keep_i)
                .unwrap()
                .roads
                .insert(r.id);
        }
        if r.dst_i == delete_i {
            r.dst_i = keep_i;
            map.intersections
                .get_mut(&keep_i)
                .unwrap()
                .roads
                .insert(r.id);
        }

        if r.src_i == keep_i && r.dst_i == keep_i {
            new_loops.push(r.id);
            map.intersections
                .get_mut(&keep_i)
                .unwrap()
                .roads
                .remove(&r.id);
        }
    }
    for r in new_loops {
        map.roads.remove(&r);
    }

    // TODO Ah, we can also wind up with multiple roads between the same intersections here. Should
    // probably auto-remove those too.

    // Restore the road geometry on the relevant side to its original length, since that can affect
    // the polygon. Note we can't just copy over the original points -- that'd clobber the other
    // side, requiring us to recalculate that polygon too.
    for id in &map.intersections[&keep_i].roads {
        map.roads.get_mut(id).unwrap().reset_pts_on_side(keep_i);
    }

    let mut i = map.intersections.get_mut(&keep_i).unwrap();
    i.polygon = geometry::intersection_polygon(i, &mut map.roads, timer);

    keep_i
}
