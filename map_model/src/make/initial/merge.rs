use crate::make::initial::{geometry, InitialMap};
use crate::raw_data::StableRoadID;
use abstutil::note;
use dimensioned::si;

pub fn short_roads(map: &mut InitialMap) {
    if false {
        // o228
        merge(map, StableRoadID(311));

        // o201
        merge(map, StableRoadID(240));

        // o37
        merge(map, StableRoadID(91));

        // o40
        merge(map, StableRoadID(59));

        // o25
        merge(map, StableRoadID(389));
        merge(map, StableRoadID(22));
    }

    if false {
        // Every time we change a road, other roads we might've already processed could shorten, so
        // we have to redo everything.
        loop {
            if let Some(r) = map
                .roads
                .values()
                .find(|r| r.trimmed_center_pts.length() < 15.0 * si::M)
            {
                merge(map, r.id);
            } else {
                break;
            }
        }
    }
}

fn merge(map: &mut InitialMap, merge_road: StableRoadID) {
    // Arbitrarily kill off the first intersection and keep the second one.
    let (delete_i, keep_i) = {
        let r = map.roads.remove(&merge_road).unwrap();
        note(format!(
            "Deleting {}, which has original length {} and trimmed length {}",
            merge_road,
            r.original_center_pts.length(),
            r.trimmed_center_pts.length()
        ));

        (r.src_i, r.dst_i)
    };
    map.intersections.remove(&delete_i);
    map.intersections
        .get_mut(&keep_i)
        .unwrap()
        .roads
        .remove(&merge_road);

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
        let r = map.roads.get_mut(id).unwrap();
        // Safe to do 'else' here, because we removed the loop roads.
        if r.dst_i == keep_i {
            if let Some(append) = r
                .original_center_pts
                .get_slice_starting_at(r.trimmed_center_pts.last_pt())
            {
                r.trimmed_center_pts = r.trimmed_center_pts.clone().extend(&append);
            }
        } else {
            if let Some(prepend) = r
                .original_center_pts
                .get_slice_ending_at(r.trimmed_center_pts.first_pt())
            {
                r.trimmed_center_pts = prepend.extend(&r.trimmed_center_pts);
            }
        }
    }
    map.save(format!("o{}_reset_roads", keep_i.0));

    let mut i = map.intersections.get_mut(&keep_i).unwrap();
    i.polygon = geometry::intersection_polygon(i, &mut map.roads);
    map.save(format!("o{}_new_polygon", keep_i.0));
}
