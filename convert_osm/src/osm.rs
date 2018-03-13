// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate map_model;
extern crate osm_xml;

use map_model::Pt2D;
use srtm;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

// TODO Result, but is there an easy way to say io error or osm xml error?
pub fn osm_to_raw_roads(osm_path: &str) -> (map_model::pb::Map, map_model::Bounds) {
    println!("Opening {}", osm_path);
    let f = File::open(osm_path).unwrap();
    let reader = BufReader::new(f);
    let doc = osm_xml::OSM::parse(reader).expect("OSM parsing failed");
    println!(
        "OSM doc has {} nodes, {} ways, {} relations",
        doc.nodes.len(),
        doc.ways.len(),
        doc.relations.len()
    );

    // resolve_reference does linear search. Let's, uh, speed that up for nodes.
    let mut id_to_node: HashMap<i64, &osm_xml::Node> = HashMap::new();
    for node in &doc.nodes {
        id_to_node.insert(node.id, node);
    }

    let mut map = map_model::pb::Map::new();
    let mut bounds = map_model::Bounds::new();
    for (i, way) in doc.ways.iter().enumerate() {
        // TODO count with a nicer progress bar
        if i % 1000 == 0 {
            println!("working on way {}/{}", i, doc.ways.len());
        }

        let mut valid = true;
        let mut pts = Vec::new();
        for node_ref in &way.nodes {
            match *node_ref {
                osm_xml::UnresolvedReference::Node(id) => match id_to_node.get(&id) {
                    Some(node) => {
                        bounds.update(node.lon, node.lat);
                        let mut pt = map_model::pb::Coordinate::new();
                        pt.set_latitude(node.lat);
                        pt.set_longitude(node.lon);
                        pts.push(pt);
                    }
                    None => {
                        valid = false;
                    }
                },
                osm_xml::UnresolvedReference::Way(id) => {
                    println!("{:?} is a nested way {:?}", node_ref, id);
                }
                osm_xml::UnresolvedReference::Relation(id) => {
                    println!("{:?} is a nested relation {:?}", node_ref, id);
                }
            }
        }
        if !valid {
            continue;
        }
        if is_road(&way.tags) {
            let mut road = map_model::pb::Road::new();
            road.set_osm_way_id(way.id);
            for tag in &way.tags {
                road.mut_osm_tags().push(format!("{}={}", tag.key, tag.val));
            }
            for pt in pts {
                road.mut_points().push(pt);
            }
            map.mut_roads().push(road);
        } else if is_bldg(&way.tags) {
            let mut bldg = map_model::pb::Building::new();
            bldg.set_osm_way_id(way.id);
            for tag in &way.tags {
                bldg.mut_osm_tags().push(format!("{}={}", tag.key, tag.val));
            }
            for pt in pts {
                bldg.mut_points().push(pt);
            }
            map.mut_buildings().push(bldg);
        }
    }
    (map, bounds)
}

pub fn split_up_roads(
    input: &map_model::pb::Map,
    elevation: &srtm::Elevation,
) -> map_model::pb::Map {
    println!("splitting up {} roads", input.get_roads().len());
    let mut counts_per_pt: HashMap<Pt2D, usize> = HashMap::new();
    let mut intersections: HashSet<Pt2D> = HashSet::new();
    for r in input.get_roads() {
        for (idx, raw_pt) in r.get_points().iter().enumerate() {
            let pt = Pt2D::from(raw_pt);
            counts_per_pt.entry(pt).or_insert(0);
            let count = counts_per_pt[&pt] + 1;
            counts_per_pt.insert(pt, count);

            if count == 2 {
                intersections.insert(pt);
            }

            // All start and endpoints of ways are also intersections.
            if idx == 0 || idx == r.get_points().len() - 1 {
                intersections.insert(pt);
            }
        }
    }

    let mut map = map_model::pb::Map::new();
    for b in input.get_buildings() {
        map.mut_buildings().push(b.clone());
    }
    for pt in &intersections {
        let mut intersection = map_model::pb::Intersection::new();
        intersection.set_has_traffic_signal(false);
        intersection.mut_point().set_longitude(pt.x());
        intersection.mut_point().set_latitude(pt.y());
        intersection.set_elevation_meters(elevation.get(pt.x(), pt.y()));
        map.mut_intersections().push(intersection);
    }

    // Now actually split up the roads based on the intersections
    for orig_road in input.get_roads() {
        let mut r = orig_road.clone();
        r.clear_points();

        for pt in orig_road.get_points() {
            r.mut_points().push(pt.clone());
            if r.get_points().len() > 1 && intersections.contains(&Pt2D::from(pt)) {
                // Start a new road
                map.mut_roads().push(r.clone());
                r.clear_points();
                r.mut_points().push(pt.clone());
            }
        }
        assert!(r.get_points().len() == 1);
    }

    // TODO we're somehow returning an intersection here with no roads. figure that out.

    map
}

fn is_road(raw_tags: &[osm_xml::Tag]) -> bool {
    let mut tags = HashMap::new();
    for tag in raw_tags {
        tags.insert(tag.key.clone(), tag.val.clone());
    }

    if !tags.contains_key("highway") {
        return false;
    }

    // https://github.com/Project-OSRM/osrm-backend/blob/master/profiles/car.lua is another
    // potential reference
    for &value in &[
        // List of non-car types from https://wiki.openstreetmap.org/wiki/Key:highway
        "living_street",
        "pedestrian",
        "track",
        "bus_guideway",
        "escape",
        "raceway",
        "footway",
        "bridleway",
        "steps",
        "path",
        "cycleway",
        "proposed",
        "construction",
        // This one's debatable. Includes alleys.
        "service",
        // more discovered manually
        "elevator",
    ] {
        if tags.get("highway") == Some(&String::from(value)) {
            return false;
        }
    }

    true
}

fn is_bldg(tags: &[osm_xml::Tag]) -> bool {
    for tag in tags {
        if tag.key == "building" {
            return true;
        }
    }
    false
}
