use crate::raw_data;
use abstutil::Timer;
use geom::HashablePt2D;
use std::collections::{HashMap, HashSet};

type IntersectionID = usize;
type RoadID = usize;

const MIN_ROAD_LENGTH_METERS: f64 = 15.0;

pub fn merge_intersections(data: &mut raw_data::Map, timer: &mut Timer) {
    timer.start_iter("merge short roads", data.roads.len());

    let mut merged = 0;
    for i in 0..data.roads.len() {
        timer.next();
        // We destroy roads and shorten this list as we go. Don't break, so the timer finishes.
        if i >= data.roads.len() {
            continue;
        }

        let mut length = 0.0;
        for pair in data.roads[i].points.windows(2) {
            length += pair[0].gps_dist_meters(pair[1]);
        }
        if length < MIN_ROAD_LENGTH_METERS {
            merge(data, i);
            merged += 1;
        }
    }

    info!("Merged {} short roads", merged);
}

fn merge(data: &mut raw_data::Map, merge_road: RoadID) {
    // Which intersection has fewer roads?
    let i1_pt = data.roads[merge_road].first_pt();
    let i2_pt = data.roads[merge_road].last_pt();
    let i1_roads: Vec<RoadID> = data
        .roads
        .iter()
        .enumerate()
        .filter(|(_, r)| r.first_pt() == i1_pt || r.last_pt() == i1_pt)
        .map(|(id, _)| id)
        .collect();
    let i2_roads: Vec<RoadID> = data
        .roads
        .iter()
        .enumerate()
        .filter(|(_, r)| r.first_pt() == i2_pt || r.last_pt() == i2_pt)
        .map(|(id, _)| id)
        .collect();
    let (delete_i_pt, extend_roads) = if i1_roads.len() < i2_roads.len() {
        (i1_pt, i1_roads)
    } else {
        (i2_pt, i2_roads)
    };

    for extend_road in extend_roads.into_iter() {
        if merge_road == extend_road {
            continue;
        }

        let new_pts = {
            let merge_r = &data.roads[merge_road];
            let extend_r = &data.roads[extend_road];

            info!("Extending r{} with r{}'s points", extend_road, merge_road);

            if merge_r.osm_tags != extend_r.osm_tags {
                let set1: HashSet<(&String, &String)> = merge_r.osm_tags.iter().collect();
                let set2: HashSet<(&String, &String)> = extend_r.osm_tags.iter().collect();
                warn!(
                    "  Losing tags {:?}, gaining tags {:?}",
                    set1.difference(&set2),
                    set2.difference(&set1)
                );
            }
            if merge_r.parking_lane_fwd != extend_r.parking_lane_fwd {
                warn!(
                    "  Overwriting parking_lane_fwd {} with {}",
                    merge_r.parking_lane_fwd, extend_r.parking_lane_fwd
                );
            }
            if merge_r.parking_lane_back != extend_r.parking_lane_back {
                warn!(
                    "  Overwriting parking_lane_back {} with {}",
                    merge_r.parking_lane_back, extend_r.parking_lane_back
                );
            }

            // TODO Clean up this awful slice mangling
            if extend_r.first_pt() == delete_i_pt {
                if merge_r.first_pt() == delete_i_pt {
                    let mut new_pts = merge_r.points.clone();
                    new_pts.reverse();
                    new_pts.pop();
                    new_pts.extend(extend_r.points.clone());
                    new_pts
                } else if merge_r.last_pt() == delete_i_pt {
                    let mut new_pts = merge_r.points.clone();
                    new_pts.pop();
                    new_pts.extend(extend_r.points.clone());
                    new_pts
                } else {
                    panic!("{:?} doesn't end at {}", merge_r, delete_i_pt);
                }
            } else if extend_r.last_pt() == delete_i_pt {
                if merge_r.first_pt() == delete_i_pt {
                    let mut new_pts = extend_r.points.clone();
                    new_pts.pop();
                    new_pts.extend(merge_r.points.clone());
                    new_pts
                } else if merge_r.last_pt() == delete_i_pt {
                    let mut new_pts = extend_r.points.clone();
                    new_pts.pop();
                    let mut rev_pts = merge_r.points.clone();
                    rev_pts.reverse();
                    new_pts.extend(rev_pts);
                    new_pts
                } else {
                    panic!("{:?} doesn't end at {}", merge_r, delete_i_pt);
                }
            } else {
                panic!("{:?} doesn't end at {}", extend_r, delete_i_pt);
            }
        };
        data.roads[extend_road].points = new_pts;
    }

    // Urgh, we need to build up lots of basic structures first.
    let mut pt_to_intersection: HashMap<HashablePt2D, IntersectionID> = HashMap::new();
    for (id, i) in data.intersections.iter().enumerate() {
        pt_to_intersection.insert(i.point.to_hashable(), id);
    }

    data.roads.remove(merge_road);
    data.intersections
        .remove(pt_to_intersection[&delete_i_pt.to_hashable()]);
}
