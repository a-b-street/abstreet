// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use crate::srtm;
use dimensioned::si;
use geom::{HashablePt2D, LonLat};
use map_model::{raw_data, IntersectionType};
use std::collections::{BTreeSet, HashMap};

pub fn split_up_roads(mut input: raw_data::Map, elevation: &srtm::Elevation) -> raw_data::Map {
    println!("splitting up {} roads", input.roads.len());

    // Look for roundabout ways. Map all points on the roundabout to a new point in the center.
    // When we process ways that touch any point on the roundabout, make them instead point to the
    // roundabout's center, so that the roundabout winds up looking like a single intersection.
    let mut remap_roundabouts: HashMap<HashablePt2D, LonLat> = HashMap::new();
    input.roads.retain(|r| {
        if r.osm_tags.get("junction") == Some(&"roundabout".to_string()) {
            let center = LonLat::center(&r.points);
            for pt in &r.points {
                remap_roundabouts.insert(pt.to_hashable(), center);
            }
            false
        } else {
            true
        }
    });

    let mut counts_per_pt: HashMap<HashablePt2D, usize> = HashMap::new();
    let mut intersections: BTreeSet<HashablePt2D> = BTreeSet::new();
    for r in input.roads.iter_mut() {
        let added_to_start = if let Some(center) = remap_roundabouts.get(&r.points[0].to_hashable())
        {
            r.points.insert(0, *center);
            true
        } else {
            false
        };
        let added_to_end =
            if let Some(center) = remap_roundabouts.get(&r.points.last().unwrap().to_hashable()) {
                r.points.push(*center);
                true
            } else {
                false
            };

        for (idx, raw_pt) in r.points.iter().enumerate() {
            let pt = raw_pt.to_hashable();
            counts_per_pt.entry(pt).or_insert(0);
            let count = counts_per_pt[&pt] + 1;
            counts_per_pt.insert(pt, count);

            if count == 2 {
                intersections.insert(pt);
            }

            // All start and endpoints of ways are also intersections.
            if idx == 0 || idx == r.points.len() - 1 {
                intersections.insert(pt);
            } else if remap_roundabouts.contains_key(&pt) {
                if idx == 1 && added_to_start {
                    continue;
                }
                if idx == r.points.len() - 2 && added_to_end {
                    continue;
                }
                panic!(
                    "OSM way {} hits a roundabout not at an endpoint. idx {} of length {}",
                    r.osm_way_id,
                    idx,
                    r.points.len()
                );
            }
        }
    }

    let mut map = raw_data::Map::blank();
    map.buildings.extend(input.buildings.clone());
    map.areas.extend(input.areas.clone());

    for pt in &intersections {
        map.intersections.push(raw_data::Intersection {
            point: LonLat::new(pt.x(), pt.y()),
            elevation: elevation.get(pt.x(), pt.y()) * si::M,
            intersection_type: IntersectionType::StopSign,
            label: None,
        });
    }

    // Now actually split up the roads based on the intersections
    for orig_road in &input.roads {
        let mut r = orig_road.clone();
        r.points.clear();

        for pt in &orig_road.points {
            r.points.push(pt.clone());
            if r.points.len() > 1 && intersections.contains(&pt.to_hashable()) {
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
