// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use geom::{Bounds, LonLat};
use map_model::raw_data;
use osm_xml;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

// TODO Result, but is there an easy way to say io error or osm xml error?
pub fn osm_to_raw_roads(osm_path: &str) -> (raw_data::Map, Bounds) {
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

    let mut map = raw_data::Map::blank();
    let mut bounds = Bounds::new();
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
                        pts.push(LonLat::new(node.lon, node.lat));
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
            map.roads.push(raw_data::Road {
                osm_way_id: way.id,
                points: pts,
                osm_tags: way.tags
                    .iter()
                    .map(|tag| (tag.key.clone(), tag.val.clone()))
                    .collect(),
            });
        } else if is_bldg(&way.tags) {
            map.buildings.push(raw_data::Building {
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
