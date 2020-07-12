use abstutil::{FileWithProgress, Timer};
use geom::{GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D, Ring};
use map_model::raw::{
    OriginalBuilding, RawArea, RawBuilding, RawBusRoute, RawBusStop, RawMap, RawParkingLot,
    RawRoad, RestrictionType,
};
use map_model::{osm, AreaType};
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
    // Simple turn restrictions: (relation ID, restriction type, from way ID, via node ID, to way
    // ID)
    Vec<(i64, RestrictionType, i64, i64, i64)>,
    // Complicated turn restrictions: (relation ID, from way ID, via way ID, to way ID)
    Vec<(i64, i64, i64, i64)>,
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
        map.boundary_polygon = Polygon::new(&gps_bounds.convert(&pts));
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
        let pt = Pt2D::from_gps(LonLat::new(node.lon, node.lat), &map.gps_bounds);
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
        if !valid || gps_pts.is_empty() {
            continue;
        }
        let pts = map.gps_bounds.convert(&gps_pts);
        let mut tags = tags_to_map(&way.tags);
        tags.insert(osm::OSM_WAY_ID.to_string(), way.id.to_string());

        if is_road(&mut tags) {
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

            map.buildings.insert(
                OriginalBuilding { osm_way_id: way.id },
                RawBuilding {
                    polygon: Polygon::new(&deduped),
                    public_garage_name: None,
                    num_parking_spots: 0,
                    amenities: get_bldg_amenities(&tags),
                    osm_tags: tags,
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
        } else if tags.get("amenity") == Some(&"parking".to_string()) {
            // TODO Verify parking = surface or handle other cases?
            map.parking_lots.push(RawParkingLot {
                polygon: Polygon::new(&pts),
                osm_id: way.id,
            });
        } else if tags.get("highway") == Some(&"service".to_string()) {
            map.parking_aisles.push(pts);
        } else {
            // The way might be part of a relation later.
            id_to_way.insert(way.id, pts);
        }
    }

    let boundary = Ring::must_new(map.boundary_polygon.points().clone());

    let mut simple_turn_restrictions = Vec::new();
    let mut complicated_turn_restrictions = Vec::new();
    timer.start_iter("processing OSM relations", doc.relations.len());
    for rel in doc.relations.values() {
        timer.next();
        let mut tags = tags_to_map(&rel.tags);
        tags.insert(osm::OSM_REL_ID.to_string(), rel.id.to_string());

        if let Some(area_type) = get_area_type(&tags) {
            if tags.get("type") == Some(&"multipolygon".to_string()) {
                if let Some(pts_per_way) = get_multipolygon_members(rel, &id_to_way) {
                    for polygon in glue_multipolygon(rel.id, pts_per_way, &boundary) {
                        map.areas.push(RawArea {
                            area_type,
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
                        simple_turn_restrictions.push((rel.id, rt, from, via, to));
                    } else if let (Some(from), Some(via), Some(to)) =
                        (from_way_id, via_way_id, to_way_id)
                    {
                        if rt == RestrictionType::BanTurns {
                            complicated_turn_restrictions.push((rel.id, from, via, to));
                        } else {
                            timer.warn(format!(
                                "Weird complicated turn restriction \"{}\" from way {} to way {} via way {}: https://www.openstreetmap.org/relation/{}",
                                restriction, from, to, via, rel.id
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
                let mut deduped = pts.clone();
                deduped.dedup();
                if deduped.len() < 3 {
                    continue;
                }
                map.buildings.insert(
                    OriginalBuilding { osm_way_id: rel.id },
                    RawBuilding {
                        polygon: Polygon::new(&deduped),
                        public_garage_name: None,
                        num_parking_spots: 0,
                        amenities: get_bldg_amenities(&tags),
                        osm_tags: tags,
                    },
                );
            }
        } else if tags.get("type") == Some(&"route_master".to_string()) {
            map.bus_routes
                .extend(extract_route(&tags, rel, &doc, &id_to_way, &map.gps_bounds));
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

    // Hack to fix z-ordering for Green Lake (and probably other places). Put water and islands
    // last. I think the more proper fix is interpreting "inner" roles in relations.
    map.areas.sort_by_key(|a| match a.area_type {
        AreaType::Island => 2,
        AreaType::Water => 1,
        _ => 0,
    });

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

fn is_road(tags: &mut BTreeMap<String, String>) -> bool {
    if tags.get("railway") == Some(&"light_rail".to_string()) {
        return true;
    }

    if !tags.contains_key(osm::HIGHWAY) {
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
        "junction",
    ] {
        if tags.get(osm::HIGHWAY) == Some(&value.to_string()) {
            return false;
        }
    }

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

    true
}

fn is_bldg(tags: &BTreeMap<String, String>) -> bool {
    // Sorry, the towers at Gasworks don't count. :)
    tags.contains_key("building") && !tags.contains_key("abandoned:man_made")
}

fn get_bldg_amenities(tags: &BTreeMap<String, String>) -> BTreeSet<(String, String)> {
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
    amenities
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

fn get_multipolygon_members(
    rel: &osm_xml::Relation,
    id_to_way: &HashMap<i64, Vec<Pt2D>>,
) -> Option<Vec<(i64, Vec<Pt2D>)>> {
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
        Some(pts_per_way)
    } else {
        None
    }
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
    if let Some(poly) = glue_to_boundary(PolyLine::must_new(result.clone()), boundary) {
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
    let hits = boundary.all_intersections(&result_pl);
    if hits.len() != 2 {
        return None;
    }

    let trimmed_result = result_pl.trim_to_endpts(hits[0], hits[1]);
    let boundary_glue = boundary.get_shorter_slice_btwn(hits[0], hits[1]);

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

fn extract_route(
    master_tags: &BTreeMap<String, String>,
    master_rel: &osm_xml::Relation,
    doc: &osm_xml::OSM,
    id_to_way: &HashMap<i64, Vec<Pt2D>>,
    gps_bounds: &GPSBounds,
) -> Option<RawBusRoute> {
    let route_name = master_tags.get("name")?.clone();
    let is_bus = match master_tags.get("route_master")?.as_ref() {
        "bus" => true,
        "light_rail" => false,
        x => {
            println!("Skipping route {} of unknown type {}", route_name, x);
            return None;
        }
    };

    let mut directions = Vec::new();
    for (_, route_member) in get_members(master_rel, doc) {
        if let osm_xml::Reference::Relation(route_rel) = route_member {
            let route_tags = tags_to_map(&route_rel.tags);
            assert_eq!(route_tags.get("type"), Some(&"route".to_string()));
            // Gather stops in order. Platforms may exist or not; match them up by name.
            let mut stops = Vec::new();
            let mut platforms = HashMap::new();
            for (role, member) in get_members(&route_rel, doc) {
                if role == "stop" {
                    if let osm_xml::Reference::Node(node) = member {
                        stops.push(RawBusStop {
                            name: tags_to_map(&node.tags)
                                .get("name")
                                .cloned()
                                .unwrap_or_else(|| format!("stop #{}", stops.len() + 1)),
                            vehicle_pos: Pt2D::from_gps(
                                LonLat::new(node.lon, node.lat),
                                gps_bounds,
                            ),
                            ped_pos: None,
                        });
                    }
                } else if role == "platform" {
                    let (platform_name, pt) = match member {
                        osm_xml::Reference::Node(node) => (
                            tags_to_map(&node.tags)
                                .get("name")
                                .cloned()
                                .unwrap_or_else(|| format!("stop #{}", platforms.len() + 1)),
                            Pt2D::from_gps(LonLat::new(node.lon, node.lat), gps_bounds),
                        ),
                        osm_xml::Reference::Way(way) => (
                            tags_to_map(&way.tags)
                                .get("name")
                                .cloned()
                                .unwrap_or_else(|| format!("stop #{}", platforms.len() + 1)),
                            if let Some(ref pts) = id_to_way.get(&way.id) {
                                Pt2D::center(pts)
                            } else {
                                continue;
                            },
                        ),
                        _ => continue,
                    };
                    platforms.insert(platform_name, pt);
                }
            }
            for stop in &mut stops {
                if let Some(pt) = platforms.remove(&stop.name) {
                    stop.ped_pos = Some(pt);
                }
            }
            if stops.len() >= 2 {
                directions.push(stops);
            }
        }
    }

    if directions.len() == 2 {
        Some(RawBusRoute {
            name: route_name,
            is_bus,
            osm_rel_id: master_rel.id,
            // The direction is arbitrary right now
            fwd_stops: directions.pop().unwrap(),
            back_stops: directions.pop().unwrap(),
        })
    } else {
        println!(
            "Skipping route {} with {} sub-routes",
            route_name,
            directions.len()
        );
        None
    }
}

// Work around osm_xml's API, which shows the node/way/relation distinction twice. This returns
// (role, resolved node/way/relation)
fn get_members<'a>(
    rel: &'a osm_xml::Relation,
    doc: &'a osm_xml::OSM,
) -> Vec<(&'a String, osm_xml::Reference<'a>)> {
    rel.members
        .iter()
        .map(|member| {
            let (id_ref, role) = match member {
                osm_xml::Member::Node(id, role)
                | osm_xml::Member::Way(id, role)
                | osm_xml::Member::Relation(id, role) => (id, role),
            };
            (role, doc.resolve_reference(id_ref))
        })
        .collect()
}
