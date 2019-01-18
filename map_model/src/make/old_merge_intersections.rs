use crate::raw_data;
use abstutil::{retain_btreemap, Timer};
use dimensioned::si;
use geom::{PolyLine, Pt2D};

pub fn old_merge_intersections(data: &mut raw_data::Map, _timer: &mut Timer) {
    if true {
        return;
    }

    // 15th and McGraw
    merge(data, raw_data::StableRoadID(59));

    // 14th and Boston
    merge(data, raw_data::StableRoadID(389));
    merge(data, raw_data::StableRoadID(22));

    if true {
        return;
    }

    // Road length effectively changes as we merge things, but not till later, so just use original
    // length.
    let gps_bounds = data.get_gps_bounds();
    let all_ids: Vec<raw_data::StableRoadID> = data.roads.keys().cloned().collect();
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
}

fn merge(data: &mut raw_data::Map, merge_road: raw_data::StableRoadID) {
    // Arbitrarily kill off the first intersection and keep the second one.
    let (delete_i, keep_i) = {
        let r = data.roads.remove(&merge_road).unwrap();
        (r.i1, r.i2)
    };
    data.intersections.remove(&delete_i);

    for r in data.roads.values_mut() {
        if r.i1 == delete_i {
            r.i1 = keep_i;
        }
        if r.i2 == delete_i {
            r.i2 = keep_i;
        }
    }

    // We might've created some loop roads on the retained intersection; remove them also.
    retain_btreemap(&mut data.roads, |_, r| r.i1 != keep_i || r.i2 != keep_i);

    // TODO Ah, we can also wind up with multiple roads between the same intersections here. Should
    // probably auto-remove those too.
}
