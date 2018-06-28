// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use map_model;
use map_model::HashablePt2D;
use osm_xml;
use srtm;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

// TODO Result, but is there an easy way to say io error or osm xml error?
pub fn osm_to_raw_roads(osm_path: &str) -> (map_model::raw_data::Map, map_model::Bounds) {
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

    let mut map = map_model::raw_data::Map::blank();
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
                        pts.push(map_model::raw_data::LonLat {
                            longitude: node.lon,
                            latitude: node.lat,
                        });
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
            map.roads.push(map_model::raw_data::Road {
                osm_way_id: way.id,
                points: pts,
                osm_tags: way.tags
                    .iter()
                    .map(|tag| (tag.key.clone(), tag.val.clone()))
                    .collect(),
            });
        } else if is_bldg(&way.tags) {
            map.buildings.push(map_model::raw_data::Building {
                osm_way_id: way.id,
                points: pts,
                osm_tags: way.tags
                    .iter()
                    .map(|tag| (tag.key.clone(), tag.val.clone()))
                    .collect(),
            });
        }
    }
    (map, bounds)
}

pub fn split_up_roads(
    input: &map_model::raw_data::Map,
    elevation: &srtm::Elevation,
) -> map_model::raw_data::Map {
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

    let mut map = map_model::raw_data::Map::blank();
    map.buildings.extend(input.buildings.clone());

    for pt in &intersections {
        map.intersections.push(map_model::raw_data::Intersection {
            point: map_model::raw_data::LonLat {
                longitude: pt.x(),
                latitude: pt.y(),
            },
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

fn hash_pt(pt: &map_model::raw_data::LonLat) -> HashablePt2D {
    HashablePt2D::new(pt.longitude, pt.latitude)
}
