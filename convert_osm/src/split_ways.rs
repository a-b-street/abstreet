// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use geom::{HashablePt2D, LonLat};
use map_model::raw_data;
use srtm;
use std::collections::{HashMap, HashSet};

pub fn split_up_roads(input: &raw_data::Map, elevation: &srtm::Elevation) -> raw_data::Map {
    println!("splitting up {} roads", input.roads.len());
    let mut counts_per_pt: HashMap<HashablePt2D, usize> = HashMap::new();
    let mut intersections: HashSet<HashablePt2D> = HashSet::new();
    for r in &input.roads {
        for (idx, raw_pt) in r.points.iter().enumerate() {
            let pt = hash_pt(raw_pt);
            counts_per_pt.entry(pt).or_insert(0);
            let count = counts_per_pt[&pt] + 1;
            counts_per_pt.insert(pt, count);

            if count == 2 {
                intersections.insert(pt);
            }

            // All start and endpoints of ways are also intersections.
            if idx == 0 || idx == r.points.len() - 1 {
                intersections.insert(pt);
            }
        }
    }

    let mut map = raw_data::Map::blank();
    map.buildings.extend(input.buildings.clone());

    for pt in &intersections {
        map.intersections.push(raw_data::Intersection {
            point: LonLat::new(pt.x(), pt.y()),
            elevation_meters: elevation.get(pt.x(), pt.y()),
            has_traffic_signal: false,
        });
    }

    // Now actually split up the roads based on the intersections
    for orig_road in &input.roads {
        let mut r = orig_road.clone();
        r.points.clear();

        for pt in &orig_road.points {
            r.points.push(pt.clone());
            if r.points.len() > 1 && intersections.contains(&hash_pt(pt)) {
                // Start a new road
                map.roads.push(r.clone());
                r.points.clear();
                r.points.push(pt.clone());
            }
        }
        assert!(r.points.len() == 1);
    }

    // TODO we're somehow returning an intersection here with no roads. figure that out.

    map
}

fn hash_pt(pt: &LonLat) -> HashablePt2D {
    HashablePt2D::new(pt.longitude, pt.latitude)
}
