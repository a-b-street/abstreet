// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil::{FileWithProgress, Timer};
use geom::LonLat;
use map_model::{raw_data, AreaType};
use osm_xml;
use std::collections::{BTreeMap, HashMap};

// TODO Result, but is there an easy way to say io error or osm xml error?
pub fn osm_to_raw_roads(osm_path: &str, timer: &mut Timer) -> raw_data::Map {
    let (reader, done) = FileWithProgress::new(osm_path).unwrap();
    let doc = osm_xml::OSM::parse(reader).expect("OSM parsing failed");
    println!(
        "OSM doc has {} nodes, {} ways, {} relations",
        doc.nodes.len(),
        doc.ways.len(),
        doc.relations.len()
    );
    done(timer);

    let mut id_to_way: HashMap<i64, Vec<LonLat>> = HashMap::new();
    let mut map = raw_data::Map::blank();
    timer.start_iter("processing OSM ways", doc.ways.len());
    for way in doc.ways.values() {
        timer.next();

        let mut valid = true;
        let mut pts = Vec::new();
        for node_ref in &way.nodes {
            match doc.resolve_reference(node_ref) {
                osm_xml::Reference::Node(node) => {
                    pts.push(LonLat::new(node.lon, node.lat));
                }
                // Don't handle nested ways/relations yet
                _ => {
                    valid = false;
                }
            }
        }
        if !valid {
            continue;
        }
        let tags = tags_to_map(&way.tags);
        if is_road(&tags) {
            map.roads.push(raw_data::Road {
                osm_way_id: way.id,
                points: pts,
                osm_tags: tags,
                // We'll fill this out later
                parking_lane_fwd: false,
                parking_lane_back: false,
            });
        } else if is_bldg(&tags) {
            map.buildings.push(raw_data::Building {
                osm_way_id: way.id,
                points: pts,
                osm_tags: tags,
            });
        } else if let Some(at) = get_area_type(&tags) {
            map.areas.push(raw_data::Area {
                area_type: at,
                osm_way_id: way.id,
                points: pts,
                osm_tags: tags,
            });
        } else {
            // The way might be part of a relation later.
            id_to_way.insert(way.id, pts);
        }
    }

    timer.start_iter("processing OSM relations", doc.relations.len());
    for rel in doc.relations.values() {
        timer.next();
        let tags = tags_to_map(&rel.tags);
        if let Some(at) = get_area_type(&tags) {
            if tags.get("type") == Some(&"multipolygon".to_string()) {
                for member in &rel.members {
                    match *member {
                        osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) => {
                            match id_to_way.get(&id) {
                                Some(pts) => {
                                    if role == "outer" {
                                        map.areas.push(raw_data::Area {
                                            area_type: at,
                                            osm_way_id: id,
                                            points: pts.to_vec(),
                                            osm_tags: tags.clone(),
                                        });
                                    } else {
                                        println!(
                                            "Relation {} has unhandled member role {}",
                                            rel.id, role
                                        );
                                    }
                                }
                                None => {
                                    println!("Relation {} refers to unknown way {}", rel.id, id);
                                }
                            }
                        }
                        _ => {
                            println!("Relation {} refers to {:?}", rel.id, member);
                        }
                    }
                }
            }
        }
    }

    map
}

fn tags_to_map(raw_tags: &[osm_xml::Tag]) -> BTreeMap<String, String> {
    raw_tags
        .iter()
        .map(|tag| (tag.key.clone(), tag.val.clone()))
        .collect()
}

fn is_road(tags: &BTreeMap<String, String>) -> bool {
    if !tags.contains_key("highway") {
        return false;
    }

    // https://github.com/Project-OSRM/osrm-backend/blob/master/profiles/car.lua is another
    // potential reference
    for &value in &[
        // List of non-car types from https://wiki.openstreetmap.org/wiki/Key:highway
        // TODO Footways are very useful, but they need more work to associate with main roads
        "footway",
        "living_street",
        "pedestrian",
        "track",
        "bus_guideway",
        "escape",
        "raceway",
        "bridleway",
        "steps",
        "path",
        "cycleway",
        "proposed",
        "construction",
        // This one's debatable. Includes alleys.
        "service",
        // more discovered manually
        "abandoned",
        "elevator",
        "planned",
    ] {
        if tags.get("highway") == Some(&String::from(value)) {
            return false;
        }
    }

    true
}

fn is_bldg(tags: &BTreeMap<String, String>) -> bool {
    tags.contains_key("building")
}

fn get_area_type(tags: &BTreeMap<String, String>) -> Option<AreaType> {
    if tags.get("leisure") == Some(&"park".to_string()) {
        return Some(AreaType::Park);
    }
    if tags.get("natural") == Some(&"wood".to_string()) {
        return Some(AreaType::Park);
    }
    if tags.get("natural") == Some(&"wetland".to_string()) {
        return Some(AreaType::Swamp);
    }
    if tags.contains_key("waterway") {
        return Some(AreaType::Water);
    }
    None
}
