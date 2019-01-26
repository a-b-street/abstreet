use crate::make::initial::{geometry, InitialMap};
use crate::raw_data::StableRoadID;
use abstutil::{note, retain_btreemap};
//use dimensioned::si;

pub fn short_roads(map: &mut InitialMap) {
    // o228
    //merge(map, StableRoadID(311));

    /*
    // o201
    merge(map, StableRoadID(240));

    // o37
    merge(map, StableRoadID(91));

    // o40
    merge(map, StableRoadID(59));

    // o25
    merge(map, StableRoadID(389));
    merge(map, StableRoadID(22));
    */

    /*
    // Road length effectively changes as we merge things, but not till later, so just use original
    // length.
    let gps_bounds = data.get_gps_bounds();
    let all_ids: Vec<StableRoadID> = data.roads.keys().cloned().collect();
    for id in all_ids {
        if let Some(r) = data.roads.get(&id) {
            let center_pts = PolyLine::new(
                r.points
                    .iter()
                    .map(|coord| Pt2D::from_gps(*coord, &gps_bounds).unwrap())
                    .collect(),
            );
            if center_pts.length() <= 15.0 * si::M {
                merge(data, id);
            }
        }
    }
    */
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
    }

    // We might've created some loop roads on the retained intersection; remove them also.
    // TODO Need to delete the references to these loops when we do this.
    /*retain_btreemap(&mut map.roads, |_, r| {
        r.src_i != keep_i || r.dst_i != keep_i
    });*/

    // TODO Ah, we can also wind up with multiple roads between the same intersections here. Should
    // probably auto-remove those too.

    // Restore the road geometry on the relevant side to its original length, since that can affect
    // the polygon. Note we can't just copy over the original points -- that'd clobber the other
    // side, requiring us to recalculate that polygon too.
    for id in &map.intersections[&keep_i].roads {
        let r = map.roads.get_mut(id).unwrap();
        // Safe to do 'else' here, because we removed the loop roads.
        if r.src_i == keep_i {
            let append = r
                .original_center_pts
                .get_slice_starting_at(r.trimmed_center_pts.last_pt());
            println!(
                "k1 {}: old trim len {}",
                r.id,
                r.trimmed_center_pts.length()
            );
            r.trimmed_center_pts = r.trimmed_center_pts.clone().extend(&append);
            println!(
                "k1 {}: new trim len {}",
                r.id,
                r.trimmed_center_pts.length()
            );
        } else {
            let prepend = r
                .original_center_pts
                .get_slice_ending_at(r.trimmed_center_pts.first_pt());
            println!(
                "k2 {}: old trim len {}",
                r.id,
                r.trimmed_center_pts.length()
            );
            r.trimmed_center_pts = prepend.extend(&r.trimmed_center_pts);
            println!(
                "k2 {}: new trim len {}",
                r.id,
                r.trimmed_center_pts.length()
            );
        }
    }

    /*let mut i = map.intersections.get_mut(&keep_i).unwrap();
    i.polygon = geometry::intersection_polygon(i, &mut map.roads);*/
}
