use crate::raw_data;
use abstutil::{retain_btreemap, Timer};

pub fn old_merge_intersections(data: &mut raw_data::Map, _timer: &mut Timer) {
    /*if true {
        return;
    }*/

    // 15th and McGraw
    merge(data, raw_data::StableRoadID(59));

    // 14th and Boston
    merge(data, raw_data::StableRoadID(389));
    merge(data, raw_data::StableRoadID(22));

    // TODO When we want to find the roads to do this automatically, we can't use original road
    // length, since it effectively changes while we delete intersections...
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
