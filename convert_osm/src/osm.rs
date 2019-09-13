use abstutil::{FileWithProgress, Timer};
use geom::{GPSBounds, HashablePt2D, LonLat, Polygon, Pt2D};
use map_model::{raw_data, AreaType};
use osm_xml;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn extract_osm(
    osm_path: &str,
    maybe_clip_path: &str,
    timer: &mut Timer,
) -> (
    raw_data::Map,
    // Un-split roads
    Vec<raw_data::Road>,
    // Traffic signals
    HashSet<HashablePt2D>,
) {
    let (reader, done) = FileWithProgress::new(osm_path).unwrap();
    let doc = osm_xml::OSM::parse(reader).expect("OSM parsing failed");
    println!(
        "OSM doc has {} nodes, {} ways, {} relations",
        doc.nodes.len(),
        doc.ways.len(),
        doc.relations.len()
    );
    done(timer);

    let mut map = if maybe_clip_path.is_empty() {
        let mut m = raw_data::Map::blank();
        for node in doc.nodes.values() {
            m.gps_bounds.update(LonLat::new(node.lon, node.lat));
        }
        m.boundary_polygon = m.gps_bounds.to_bounds().get_rectangle();
        m
    } else {
        read_osmosis_polygon(maybe_clip_path)
    };

    let mut id_to_way: HashMap<i64, Vec<Pt2D>> = HashMap::new();
    let mut roads: Vec<raw_data::Road> = Vec::new();
    let mut traffic_signals: HashSet<HashablePt2D> = HashSet::new();

    timer.start_iter("processing OSM nodes", doc.nodes.len());
    for node in doc.nodes.values() {
        timer.next();
        let tags = tags_to_map(&node.tags);
        if tags.get("highway") == Some(&"traffic_signals".to_string()) {
            traffic_signals.insert(
                Pt2D::forcibly_from_gps(LonLat::new(node.lon, node.lat), &map.gps_bounds)
                    .to_hashable(),
            );
        }
    }

    timer.start_iter("processing OSM ways", doc.ways.len());
    for way in doc.ways.values() {
        timer.next();

        let mut valid = true;
        let mut gps_pts = Vec::new();
        for node_ref in &way.nodes {
            match doc.resolve_reference(node_ref) {
                osm_xml::Reference::Node(node) => {
                    gps_pts.push(LonLat::new(node.lon, node.lat));
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
        let pts = map.gps_bounds.forcibly_convert(&gps_pts);
        let tags = tags_to_map(&way.tags);
        if is_road(&tags) {
            roads.push(raw_data::Road {
                osm_way_id: way.id,
                center_points: pts,
                orig_id: raw_data::OriginalRoad {
                    pt1: gps_pts[0],
                    pt2: *gps_pts.last().unwrap(),
                },
                osm_tags: tags,
                // We'll fill this out later
                i1: raw_data::StableIntersectionID(0),
                i2: raw_data::StableIntersectionID(0),
                parking_lane_fwd: false,
                parking_lane_back: false,
            });
        } else if is_bldg(&tags) {
            let deduped = Pt2D::approx_dedupe(pts, geom::EPSILON_DIST);
            if deduped.len() < 3 {
                continue;
            }
            map.buildings.push(raw_data::Building {
                osm_way_id: way.id,
                polygon: Polygon::new(&deduped),
                osm_tags: tags,
                parking: None,
            });
        } else if let Some(at) = get_area_type(&tags) {
            if pts.len() < 3 {
                continue;
            }
            map.areas.push(raw_data::Area {
                area_type: at,
                osm_id: way.id,
                polygon: Polygon::new(&pts),
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
                let mut ok = true;
                let mut pts_per_way: Vec<Vec<Pt2D>> = Vec::new();
                for member in &rel.members {
                    match member {
                        osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) => {
                            // If the way is clipped out, that's fine
                            if let Some(pts) = id_to_way.get(id) {
                                if role == "outer" {
                                    pts_per_way.push(pts.to_vec());
                                } else {
                                    println!(
                                        "Relation {} has unhandled member role {}, ignoring it",
                                        rel.id, role
                                    );
                                }
                            }
                        }
                        _ => {
                            println!("Relation {} refers to {:?}", rel.id, member);
                            ok = false;
                        }
                    }
                }
                if ok {
                    let polygons = glue_multipolygon(pts_per_way);
                    if polygons.is_empty() {
                        println!("Relation {} failed to glue multipolygon", rel.id);
                    } else {
                        for polygon in polygons {
                            map.areas.push(raw_data::Area {
                                area_type: at,
                                osm_id: rel.id,
                                polygon,
                                osm_tags: tags.clone(),
                            });
                        }
                    }
                }
            }
        } else if tags.get("type") == Some(&"restriction".to_string()) {
            let mut from_way_id: Option<i64> = None;
            let mut to_way_id: Option<i64> = None;
            for member in &rel.members {
                if let osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) =
                    member
                {
                    if role == "from" {
                        from_way_id = Some(*id);
                    } else if role == "to" {
                        to_way_id = Some(*id);
                    }
                }
            }
            if let (Some(from_way_id), Some(to_way_id)) = (from_way_id, to_way_id) {
                if let Some(restriction) = tags.get("restriction") {
                    map.turn_restrictions
                        .entry(from_way_id)
                        .or_insert_with(Vec::new)
                        .push((restriction.to_string(), to_way_id));
                }
            }
        }
    }

    (map, roads, traffic_signals)
}

fn tags_to_map(raw_tags: &[osm_xml::Tag]) -> BTreeMap<String, String> {
    raw_tags
        .iter()
        .filter_map(|tag| {
            // Toss out really useless metadata.
            if tag.key.starts_with("tiger:") || tag.key.starts_with("old_name:") {
                None
            } else {
                Some((tag.key.clone(), tag.val.clone()))
            }
        })
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
        "razed",
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
    if tags.get("leisure") == Some(&"golf_course".to_string()) {
        return Some(AreaType::Park);
    }
    if tags.get("natural") == Some(&"wood".to_string()) {
        return Some(AreaType::Park);
    }
    if tags.get("landuse") == Some(&"cemetery".to_string()) {
        return Some(AreaType::Park);
    }
    if tags.get("natural") == Some(&"water".to_string()) {
        return Some(AreaType::Water);
    }
    None
}

// The result could be more than one disjoint polygon.
fn glue_multipolygon(mut pts_per_way: Vec<Vec<Pt2D>>) -> Vec<Polygon> {
    // First deal with all of the closed loops.
    let mut polygons: Vec<Polygon> = Vec::new();
    pts_per_way.retain(|pts| {
        if pts[0] == *pts.last().unwrap() {
            polygons.push(Polygon::new(pts));
            false
        } else {
            true
        }
    });
    if pts_per_way.is_empty() {
        return polygons;
    }

    // The main polygon
    let mut result = pts_per_way.pop().unwrap();
    let mut reversed = false;
    while !pts_per_way.is_empty() {
        let glue_pt = *result.last().unwrap();
        if let Some(idx) = pts_per_way
            .iter()
            .position(|pts| pts[0] == glue_pt || *pts.last().unwrap() == glue_pt)
        {
            let mut append = pts_per_way.remove(idx);
            if append[0] != glue_pt {
                append.reverse();
            }
            result.pop();
            result.extend(append);
        } else {
            if reversed {
                // Totally filter the thing out, since something clearly broke.
                return Vec::new();
            } else {
                reversed = true;
                result.reverse();
                // Try again!
            }
        }
    }

    // Some ways of the multipolygon are clipped out. Connect the ends in the most straightforward
    // way. Later polygon clipping will trim to the boundary.
    if result[0] != *result.last().unwrap() {
        result.push(result[0]);
    }
    polygons.push(Polygon::new(&result));
    polygons
}

fn read_osmosis_polygon(path: &str) -> raw_data::Map {
    let mut pts: Vec<LonLat> = Vec::new();
    let mut gps_bounds = GPSBounds::new();
    for (idx, maybe_line) in BufReader::new(File::open(path).unwrap())
        .lines()
        .enumerate()
    {
        if idx == 0 || idx == 1 {
            continue;
        }
        let line = maybe_line.unwrap();
        if line == "END" {
            break;
        }
        let parts: Vec<&str> = line.trim_start().split("    ").collect();
        assert!(parts.len() == 2);
        let pt = LonLat::new(
            parts[0].parse::<f64>().unwrap(),
            parts[1].parse::<f64>().unwrap(),
        );
        pts.push(pt);
        gps_bounds.update(pt);
    }

    let mut map = raw_data::Map::blank();
    map.boundary_polygon = Polygon::new(&gps_bounds.must_convert(&pts));
    map.gps_bounds = gps_bounds;
    map
}
