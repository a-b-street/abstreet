use abstutil::{FileWithProgress, Timer};
use geom::{GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D, Ring};
use map_model::raw::{OriginalBuilding, RawArea, RawBuilding, RawMap, RawRoad, RestrictionType};
use map_model::{osm, AreaType};
use osm_xml;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub fn extract_osm(
    osm_path: &str,
    maybe_clip_path: &Option<String>,
    city_name: &str,
    map_name: &str,
    timer: &mut Timer,
) -> (
    RawMap,
    // Un-split roads
    Vec<(i64, RawRoad)>,
    // Traffic signals
    HashSet<HashablePt2D>,
    // OSM Node IDs
    HashMap<HashablePt2D, i64>,
    // Simple turn restrictions: (restriction type, from way ID, via node ID, to way ID)
    Vec<(RestrictionType, i64, i64, i64)>,
    // Complicated turn restrictions: (from way ID, via way ID, to way ID)
    Vec<(i64, i64, i64)>,
    // Amenities (location, name, amenity type)
    Vec<(Pt2D, String, String)>,
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

    let mut map = if let Some(path) = maybe_clip_path {
        let pts = LonLat::read_osmosis_polygon(path.to_string()).unwrap();
        let mut gps_bounds = GPSBounds::new();
        for pt in &pts {
            gps_bounds.update(*pt);
        }

        let mut map = RawMap::blank(city_name, map_name);
        map.boundary_polygon = Polygon::new(&gps_bounds.must_convert(&pts));
        map.gps_bounds = gps_bounds;
        map
    } else {
        let mut m = RawMap::blank(city_name, map_name);
        for node in doc.nodes.values() {
            m.gps_bounds.update(LonLat::new(node.lon, node.lat));
        }
        m.boundary_polygon = m.gps_bounds.to_bounds().get_rectangle();
        m
    };

    let mut id_to_way: HashMap<i64, Vec<Pt2D>> = HashMap::new();
    let mut roads: Vec<(i64, RawRoad)> = Vec::new();
    let mut traffic_signals: HashSet<HashablePt2D> = HashSet::new();
    let mut osm_node_ids = HashMap::new();
    let mut node_amenities = Vec::new();

    timer.start_iter("processing OSM nodes", doc.nodes.len());
    for node in doc.nodes.values() {
        timer.next();
        let pt = Pt2D::forcibly_from_gps(LonLat::new(node.lon, node.lat), &map.gps_bounds);
        osm_node_ids.insert(pt.to_hashable(), node.id);

        let tags = tags_to_map(&node.tags);
        if tags.get(osm::HIGHWAY) == Some(&"traffic_signals".to_string()) {
            traffic_signals.insert(pt.to_hashable());
        }
        if let Some(amenity) = tags.get("amenity") {
            node_amenities.push((
                pt,
                tags.get("name")
                    .cloned()
                    .unwrap_or_else(|| "unnamed".to_string()),
                amenity.clone(),
            ));
        }
        if let Some(shop) = tags.get("shop") {
            node_amenities.push((
                pt,
                tags.get("name")
                    .cloned()
                    .unwrap_or_else(|| "unnamed".to_string()),
                shop.clone(),
            ));
        }
    }

    let mut coastline_groups: Vec<(i64, Vec<Pt2D>)> = Vec::new();
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
        let mut tags = tags_to_map(&way.tags);
        tags.insert(osm::OSM_WAY_ID.to_string(), way.id.to_string());

        if is_road(&tags) {
            // If there's no parking data in OSM already, then assume no parking and mark that it's
            // inferred.
            if !tags.contains_key(osm::PARKING_LEFT)
                && !tags.contains_key(osm::PARKING_RIGHT)
                && !tags.contains_key(osm::PARKING_BOTH)
                && tags.get(osm::HIGHWAY) != Some(&"motorway".to_string())
                && tags.get(osm::HIGHWAY) != Some(&"motorway_link".to_string())
                && tags.get("junction") != Some(&"roundabout".to_string())
            {
                tags.insert(osm::PARKING_BOTH.to_string(), "no_parking".to_string());
                tags.insert(osm::INFERRED_PARKING.to_string(), "true".to_string());
            }

            // If there's no sidewalk data in OSM already, then make an assumption and mark that
            // it's inferred.
            if !tags.contains_key(osm::SIDEWALK) {
                tags.insert(osm::INFERRED_SIDEWALKS.to_string(), "true".to_string());
                if tags.get(osm::HIGHWAY) == Some(&"motorway".to_string())
                    || tags.get(osm::HIGHWAY) == Some(&"motorway_link".to_string())
                    || tags.get("junction") == Some(&"roundabout".to_string())
                {
                    tags.insert(osm::SIDEWALK.to_string(), "none".to_string());
                } else if tags.get("oneway") == Some(&"yes".to_string()) {
                    tags.insert(osm::SIDEWALK.to_string(), "right".to_string());
                    if tags.get(osm::HIGHWAY) == Some(&"residential".to_string()) {
                        tags.insert(osm::SIDEWALK.to_string(), "both".to_string());
                    }
                } else {
                    tags.insert(osm::SIDEWALK.to_string(), "both".to_string());
                }
            }

            // TODO Hardcoding these overrides. OSM is correct, these don't have
            // sidewalks; there's a crosswalk mapped. But until we can snap sidewalks properly, do
            // this to prevent the sidewalks from being disconnected.
            if way.id == 332060260 || way.id == 332060236 {
                tags.insert(osm::SIDEWALK.to_string(), "right".to_string());
            }

            roads.push((
                way.id,
                RawRoad {
                    center_points: pts,
                    osm_tags: tags,
                    turn_restrictions: Vec::new(),
                    complicated_turn_restrictions: Vec::new(),
                },
            ));
        } else if is_bldg(&tags) {
            let mut deduped = pts.clone();
            deduped.dedup();
            if deduped.len() < 3 {
                continue;
            }

            let mut amenities = BTreeSet::new();
            if let Some(amenity) = tags.get("amenity") {
                amenities.insert((
                    tags.get("name")
                        .cloned()
                        .unwrap_or_else(|| "unnamed".to_string()),
                    amenity.clone(),
                ));
            }
            if let Some(shop) = tags.get("shop") {
                amenities.insert((
                    tags.get("name")
                        .cloned()
                        .unwrap_or_else(|| "unnamed".to_string()),
                    shop.clone(),
                ));
            }
            map.buildings.insert(
                OriginalBuilding { osm_way_id: way.id },
                RawBuilding {
                    polygon: Polygon::new(&deduped),
                    osm_tags: tags,
                    public_garage_name: None,
                    num_parking_spots: 0,
                    amenities,
                },
            );
        } else if let Some(at) = get_area_type(&tags) {
            if pts.len() < 3 {
                continue;
            }
            map.areas.push(RawArea {
                area_type: at,
                osm_id: way.id,
                polygon: Polygon::new(&pts),
                osm_tags: tags,
            });
        } else if tags.get("natural") == Some(&"coastline".to_string()) {
            coastline_groups.push((way.id, pts));
        } else {
            // The way might be part of a relation later.
            id_to_way.insert(way.id, pts);
        }
    }

    let boundary = Ring::new(map.boundary_polygon.points().clone());

    let mut simple_turn_restrictions = Vec::new();
    let mut complicated_turn_restrictions = Vec::new();
    timer.start_iter("processing OSM relations", doc.relations.len());
    for rel in doc.relations.values() {
        timer.next();
        let mut tags = tags_to_map(&rel.tags);
        tags.insert(osm::OSM_REL_ID.to_string(), rel.id.to_string());
        if let Some(at) = get_area_type(&tags) {
            if tags.get("type") == Some(&"multipolygon".to_string()) {
                let mut ok = true;
                let mut pts_per_way: Vec<(i64, Vec<Pt2D>)> = Vec::new();
                for member in &rel.members {
                    match member {
                        osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) => {
                            // If the way is clipped out, that's fine
                            if let Some(pts) = id_to_way.get(id) {
                                if role == "outer" {
                                    pts_per_way.push((*id, pts.to_vec()));
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
                    for polygon in glue_multipolygon(rel.id, pts_per_way, &boundary) {
                        map.areas.push(RawArea {
                            area_type: at,
                            osm_id: rel.id,
                            polygon,
                            osm_tags: tags.clone(),
                        });
                    }
                }
            }
        } else if tags.get("type") == Some(&"restriction".to_string()) {
            let mut from_way_id: Option<i64> = None;
            let mut via_node_id: Option<i64> = None;
            let mut via_way_id: Option<i64> = None;
            let mut to_way_id: Option<i64> = None;
            for member in &rel.members {
                match member {
                    osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) => {
                        if role == "from" {
                            from_way_id = Some(*id);
                        } else if role == "to" {
                            to_way_id = Some(*id);
                        } else if role == "via" {
                            via_way_id = Some(*id);
                        }
                    }
                    osm_xml::Member::Node(osm_xml::UnresolvedReference::Node(id), ref role) => {
                        if role == "via" {
                            via_node_id = Some(*id);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            if let Some(restriction) = tags.get("restriction") {
                if let Some(rt) = RestrictionType::new(restriction) {
                    if let (Some(from), Some(via), Some(to)) = (from_way_id, via_node_id, to_way_id)
                    {
                        simple_turn_restrictions.push((rt, from, via, to));
                    } else if let (Some(from), Some(via), Some(to)) =
                        (from_way_id, via_way_id, to_way_id)
                    {
                        if rt == RestrictionType::BanTurns {
                            complicated_turn_restrictions.push((from, via, to));
                        } else {
                            timer.warn(format!(
                                "Weird complicated turn restriction from {} to {} via {}: {}",
                                from, to, via, restriction
                            ));
                        }
                    }
                }
            }
        } else if is_bldg(&tags) {
            if let Some(pts) = rel
                .members
                .iter()
                .filter_map(|x| match x {
                    osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) => {
                        if role == "outer" {
                            Some(*id)
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .next()
                .and_then(|id| id_to_way.get(&id))
            {
                // TODO Dedupe code
                let mut amenities = BTreeSet::new();
                if let Some(amenity) = tags.get("amenity") {
                    amenities.insert((
                        tags.get("name")
                            .cloned()
                            .unwrap_or_else(|| "unnamed".to_string()),
                        amenity.clone(),
                    ));
                }
                if let Some(shop) = tags.get("shop") {
                    amenities.insert((
                        tags.get("name")
                            .cloned()
                            .unwrap_or_else(|| "unnamed".to_string()),
                        shop.clone(),
                    ));
                }
                if pts.len() < 3 {
                    continue;
                }
                map.buildings.insert(
                    OriginalBuilding { osm_way_id: rel.id },
                    RawBuilding {
                        polygon: Polygon::new(pts),
                        osm_tags: tags,
                        public_garage_name: None,
                        num_parking_spots: 0,
                        amenities,
                    },
                );
            }
        }
    }

    // Special case the coastline.
    println!("{} ways of coastline", coastline_groups.len());
    for polygon in glue_multipolygon(-1, coastline_groups, &boundary) {
        let mut osm_tags = BTreeMap::new();
        osm_tags.insert("water".to_string(), "ocean".to_string());
        // Put it at the beginning, so that it's naturally beneath island areas
        map.areas.insert(
            0,
            RawArea {
                area_type: AreaType::Water,
                osm_id: -1,
                polygon,
                osm_tags,
            },
        );
    }

    (
        map,
        roads,
        traffic_signals,
        osm_node_ids,
        simple_turn_restrictions,
        complicated_turn_restrictions,
        node_amenities,
    )
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
    if !tags.contains_key(osm::HIGHWAY) {
        return false;
    }
    // TODO Need to figure out how to ban cutting through in the contraction hierarchy.
    if tags.get("access") == Some(&"private".to_string()) {
        return false;
    }
    if tags.get("access") == Some(&"no".to_string())
        && tags.get("bus") != Some(&"yes".to_string())
        && tags.get("highway") != Some(&"construction".to_string())
    {
        return false;
    }

    // https://github.com/Project-OSRM/osrm-backend/blob/master/profiles/car.lua is another
    // potential reference
    for value in &[
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
        // This one's debatable. Includes alleys.
        "service",
        // more discovered manually
        "abandoned",
        "elevator",
        "planned",
        "razed",
        "corridor",
    ] {
        if tags.get(osm::HIGHWAY) == Some(&value.to_string()) {
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
    if tags.get("natural") == Some(&"water".to_string())
        || tags.get("waterway") == Some(&"riverbank".to_string())
    {
        return Some(AreaType::Water);
    }
    if tags.get("place") == Some(&"island".to_string()) {
        return Some(AreaType::Island);
    }
    // TODO These just cover up poorly inferred road geometry now. Figure out how to use these.
    if false {
        if tags.get("traffic_calming") == Some(&"island".to_string()) {
            return Some(AreaType::PedestrianIsland);
        }
        if tags.get("highway") == Some(&"pedestrian".to_string())
            && tags.get("area") == Some(&"yes".to_string())
        {
            return Some(AreaType::PedestrianIsland);
        }
    }
    None
}

// The result could be more than one disjoint polygon.
fn glue_multipolygon(
    rel_id: i64,
    mut pts_per_way: Vec<(i64, Vec<Pt2D>)>,
    boundary: &Ring,
) -> Vec<Polygon> {
    // First deal with all of the closed loops.
    let mut polygons: Vec<Polygon> = Vec::new();
    pts_per_way.retain(|(_, pts)| {
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
    let (_, mut result) = pts_per_way.pop().unwrap();
    let mut reversed = false;
    while !pts_per_way.is_empty() {
        let glue_pt = *result.last().unwrap();
        if let Some(idx) = pts_per_way
            .iter()
            .position(|(_, pts)| pts[0] == glue_pt || *pts.last().unwrap() == glue_pt)
        {
            let (_, mut append) = pts_per_way.remove(idx);
            if append[0] != glue_pt {
                append.reverse();
            }
            result.pop();
            result.extend(append);
        } else {
            if reversed {
                // TODO Investigate what's going on here. At the very least, take what we have so
                // far and try to glue it up.
                println!(
                    "Throwing away {} chunks from relation {}: ways {:?}",
                    pts_per_way.len(),
                    rel_id,
                    pts_per_way.iter().map(|(id, _)| *id).collect::<Vec<i64>>()
                );
                break;
            } else {
                reversed = true;
                result.reverse();
                // Try again!
            }
        }
    }

    if result[0] == *result.last().unwrap() {
        polygons.push(Polygon::new(&result));
        return polygons;
    }
    if let Some(poly) = glue_to_boundary(PolyLine::new(result.clone()), boundary) {
        polygons.push(poly);
    } else {
        // Give up and just connect the ends directly.
        result.push(result[0]);
        polygons.push(Polygon::new(&result));
    }

    polygons
}

fn glue_to_boundary(result_pl: PolyLine, boundary: &Ring) -> Option<Polygon> {
    // Some ways of the multipolygon must be clipped out. First try to trace along the boundary.
    let hit1 = boundary.first_intersection(&result_pl)?;
    let hit2 = boundary.first_intersection(&result_pl.reversed())?;
    if hit1 == hit2 {
        return None;
    }

    let trimmed_result = result_pl.trim_to_endpts(hit1, hit2);
    let boundary_glue = boundary.get_shorter_slice_btwn(hit1, hit2);

    let mut trimmed_pts = trimmed_result.points().clone();
    if trimmed_result.last_pt() == boundary_glue.first_pt() {
        trimmed_pts.pop();
        trimmed_pts.extend(boundary_glue.points().clone());
    } else {
        assert_eq!(trimmed_result.last_pt(), boundary_glue.last_pt());
        trimmed_pts.pop();
        trimmed_pts.extend(boundary_glue.reversed().points().clone());
    }
    assert_eq!(trimmed_pts[0], *trimmed_pts.last().unwrap());
    Some(Polygon::new(&trimmed_pts))
}
