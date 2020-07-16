use abstutil::{retain_btreemap, FileWithProgress, Tags, Timer};
use geom::{GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D, Ring};
use map_model::raw::{
    OriginalBuilding, OriginalIntersection, RawArea, RawBuilding, RawBusRoute, RawBusStop, RawMap,
    RawParkingLot, RawRoad, RestrictionType,
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
        if tags.is(osm::HIGHWAY, "traffic_signals") {
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
    let mut memorial_areas: Vec<Polygon> = Vec::new();
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
        tags.insert(osm::OSM_WAY_ID, way.id.to_string());

        if is_road(&mut tags) {
            // TODO Hardcoding these overrides. OSM is correct, these don't have
            // sidewalks; there's a crosswalk mapped. But until we can snap sidewalks properly, do
            // this to prevent the sidewalks from being disconnected.
            if way.id == 332060260 || way.id == 332060236 {
                tags.insert(osm::SIDEWALK, "right");
            }

            roads.push((
                way.id,
                RawRoad {
                    center_points: pts,
                    osm_tags: tags.take(),
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
                    osm_tags: tags.take(),
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
                osm_tags: tags.take(),
            });
        } else if tags.is("natural", "coastline") {
            coastline_groups.push((way.id, pts));
        } else if tags.is("amenity", "parking") {
            // TODO Verify parking = surface or handle other cases?
            map.parking_lots.push(RawParkingLot {
                polygon: Polygon::new(&pts),
                osm_id: way.id,
            });
        } else if tags.is("highway", "service") {
            map.parking_aisles.push(pts);
        } else if tags.is("historic", "memorial") {
            if pts[0] == *pts.last().unwrap() {
                memorial_areas.push(Polygon::new(&pts));
            }
        } else {
            // The way might be part of a relation later.
            id_to_way.insert(way.id, pts);
        }
    }

    let boundary = Ring::must_new(map.boundary_polygon.points().clone());

    let mut simple_turn_restrictions = Vec::new();
    let mut complicated_turn_restrictions = Vec::new();
    let mut amenity_areas: Vec<(String, String, Polygon)> = Vec::new();
    // Vehicle position (stop) -> pedestrian position (platform)
    let mut stop_areas: Vec<(Pt2D, Pt2D)> = Vec::new();
    timer.start_iter("processing OSM relations", doc.relations.len());
    for rel in doc.relations.values() {
        timer.next();
        let mut tags = tags_to_map(&rel.tags);
        tags.insert(osm::OSM_REL_ID, rel.id.to_string());

        if let Some(area_type) = get_area_type(&tags) {
            if tags.is("type", "multipolygon") {
                if let Some(pts_per_way) = get_multipolygon_members(rel, &id_to_way) {
                    for polygon in glue_multipolygon(rel.id, pts_per_way, &boundary) {
                        map.areas.push(RawArea {
                            area_type,
                            osm_id: rel.id,
                            polygon,
                            osm_tags: tags.clone().take(),
                        });
                    }
                }
            }
        } else if tags.is("type", "restriction") {
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
                                "Weird complicated turn restriction \"{}\" from way {} to way {} \
                                 via way {}: {}",
                                restriction,
                                from,
                                to,
                                via,
                                rel_url(rel.id)
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
                        osm_tags: tags.take(),
                    },
                );
            }
        } else if tags.is("type", "route") {
            map.bus_routes.extend(extract_route(
                &tags,
                rel,
                &doc,
                &id_to_way,
                &map.gps_bounds,
                &map.boundary_polygon,
            ));
        } else if tags.is("type", "multipolygon") && tags.contains_key("amenity") {
            let name = tags
                .get("name")
                .cloned()
                .unwrap_or_else(|| "unnamed".to_string());
            let amenity = tags.get("amenity").clone().unwrap();
            for member in &rel.members {
                if let osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), ref role) =
                    member
                {
                    if role != "outer" {
                        continue;
                    }
                    if let Some(pts) = id_to_way.get(&id) {
                        if pts[0] == *pts.last().unwrap() {
                            amenity_areas.push((name.clone(), amenity.clone(), Polygon::new(pts)));
                        }
                    }
                }
            }
        } else if tags.is("public_transport", "stop_area") {
            let mut stops = Vec::new();
            let mut platform: Option<Pt2D> = None;
            for (role, member) in get_members(rel, &doc) {
                if let osm_xml::Reference::Node(node) = member {
                    let pt = Pt2D::from_gps(LonLat::new(node.lon, node.lat), &map.gps_bounds);
                    if role == "stop" {
                        stops.push(pt);
                    } else if role == "platform" {
                        platform = Some(pt);
                    }
                } else if let osm_xml::Reference::Way(way) = member {
                    if role == "platform" {
                        if let Some(pts) = id_to_way.get(&way.id) {
                            platform = Some(Pt2D::center(pts));
                        }
                    }
                }
            }
            if let Some(ped_pos) = platform {
                for vehicle_pos in stops {
                    stop_areas.push((vehicle_pos, ped_pos));
                }
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

    // Berlin has lots of "buildings" mapped in the Holocaust-Mahnmal. Filter them out.
    timer.start_iter("match buildings to memorial areas", memorial_areas.len());
    for area in memorial_areas {
        timer.next();
        retain_btreemap(&mut map.buildings, |_, b| {
            !area.contains_pt(b.polygon.center())
        });
    }

    timer.start_iter("match buildings to amenity areas", amenity_areas.len());
    for (name, amenity, poly) in amenity_areas {
        for b in map.buildings.values_mut() {
            if poly.contains_pt(b.polygon.center()) {
                b.amenities.insert((name.clone(), amenity.clone()));
            }
        }
    }

    // Match platforms from stop_areas. Not sure what order routes and stop_areas will appear in
    // relations, so do this after reading all of them.
    for (vehicle_pos, ped_pos) in stop_areas {
        for route in &mut map.bus_routes {
            for stop in &mut route.stops {
                if stop.vehicle_pos == vehicle_pos {
                    stop.ped_pos = Some(ped_pos);
                }
            }
        }
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

fn tags_to_map(raw_tags: &[osm_xml::Tag]) -> Tags {
    Tags::new(
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
            .collect(),
    )
}

fn is_road(tags: &mut Tags) -> bool {
    if tags.is("railway", "light_rail") {
        return true;
    }

    if !tags.contains_key(osm::HIGHWAY) {
        return false;
    }

    // https://github.com/Project-OSRM/osrm-backend/blob/master/profiles/car.lua is another
    // potential reference
    if tags.is_any(
        osm::HIGHWAY,
        vec![
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
            "bus_stop",
        ],
    ) {
        return false;
    }

    // If there's no parking data in OSM already, then assume no parking and mark that it's
    // inferred.
    if !tags.contains_key(osm::PARKING_LEFT)
        && !tags.contains_key(osm::PARKING_RIGHT)
        && !tags.contains_key(osm::PARKING_BOTH)
        && !tags.is(osm::HIGHWAY, "motorway")
        && !tags.is(osm::HIGHWAY, "motorway_link")
        && !tags.is("junction", "roundabout")
    {
        tags.insert(osm::PARKING_BOTH, "no_parking");
        tags.insert(osm::INFERRED_PARKING, "true");
    }

    // If there's no sidewalk data in OSM already, then make an assumption and mark that
    // it's inferred.
    if !tags.contains_key(osm::SIDEWALK) {
        tags.insert(osm::INFERRED_SIDEWALKS, "true");
        if tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
            || tags.is("junction", "roundabout")
        {
            tags.insert(osm::SIDEWALK, "none");
        } else if tags.is("oneway", "yes") {
            tags.insert(osm::SIDEWALK, "right");
            if tags.is(osm::HIGHWAY, "residential") {
                tags.insert(osm::SIDEWALK, "both");
            }
        } else {
            tags.insert(osm::SIDEWALK, "both");
        }
    }

    true
}

fn is_bldg(tags: &Tags) -> bool {
    // Sorry, the towers at Gasworks don't count. :)
    tags.contains_key("building") && !tags.contains_key("abandoned:man_made")
}

fn get_bldg_amenities(tags: &Tags) -> BTreeSet<(String, String)> {
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

fn get_area_type(tags: &Tags) -> Option<AreaType> {
    if tags.is_any("leisure", vec!["park", "golf_course"]) {
        return Some(AreaType::Park);
    }
    if tags.is("natural", "wood") {
        return Some(AreaType::Park);
    }
    if tags.is("landuse", "cemetery") {
        return Some(AreaType::Park);
    }

    if tags.is("natural", "water") || tags.is("waterway", "riverbank") {
        return Some(AreaType::Water);
    }

    if tags.is("place", "island") {
        return Some(AreaType::Island);
    }

    // TODO These just cover up poorly inferred road geometry now. Figure out how to use these.
    if false {
        if tags.is("traffic_calming", "island") {
            return Some(AreaType::PedestrianIsland);
        }
        if tags.is("highway", "pedestrian") && tags.is("area", "yes") {
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
                            "{} has unhandled member role {}, ignoring it",
                            rel_url(rel.id),
                            role
                        );
                    }
                }
            }
            _ => {
                println!("{} refers to {:?}", rel_url(rel.id), member);
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
    tags: &Tags,
    rel: &osm_xml::Relation,
    doc: &osm_xml::OSM,
    id_to_way: &HashMap<i64, Vec<Pt2D>>,
    gps_bounds: &GPSBounds,
    boundary: &Polygon,
) -> Option<RawBusRoute> {
    let full_name = tags.get("name")?.clone();
    let short_name = tags
        .get("ref")
        .cloned()
        .unwrap_or_else(|| full_name.clone());
    let is_bus = match tags.get("route")?.as_ref() {
        "bus" => true,
        "light_rail" => false,
        x => {
            if x != "road" && x != "bicycle" {
                // TODO Handle these at some point
                println!(
                    "Skipping route {} of unknown type {}: {}",
                    full_name,
                    x,
                    rel_url(rel.id)
                );
            }
            return None;
        }
    };

    // Gather stops in order. Platforms may exist or not; match them up by name.
    let mut stops = Vec::new();
    let mut platforms = HashMap::new();
    let mut all_pts = Vec::new();
    for (role, member) in get_members(rel, doc) {
        if role == "stop" {
            if let osm_xml::Reference::Node(node) = member {
                stops.push(RawBusStop {
                    name: tags_to_map(&node.tags)
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| format!("stop #{}", stops.len() + 1)),
                    vehicle_pos: Pt2D::from_gps(LonLat::new(node.lon, node.lat), gps_bounds),
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
        } else if let osm_xml::Reference::Way(way) = member {
            // The order of nodes might be wrong, doesn't matter
            for node in &way.nodes {
                if let osm_xml::UnresolvedReference::Node(id) = node {
                    all_pts.push(OriginalIntersection { osm_node_id: *id });
                }
            }
        }
    }
    for stop in &mut stops {
        if let Some(pt) = platforms.remove(&stop.name) {
            stop.ped_pos = Some(pt);
        }
    }

    // Remove stops that're out of bounds. Once we find the first in-bound point, keep all in-bound
    // stops and halt as soon as we go out of bounds again. If a route happens to dip in and out of
    // the boundary, we don't want to leave gaps.
    let mut keep_stops = Vec::new();
    let orig_num = stops.len();
    for stop in stops {
        if boundary.contains_pt(stop.vehicle_pos) {
            keep_stops.push(stop);
        } else {
            if !keep_stops.is_empty() {
                // That's the end of them
                break;
            }
        }
    }
    println!(
        "Kept {} / {} contiguous stops from route {}",
        keep_stops.len(),
        orig_num,
        rel_url(rel.id)
    );

    if keep_stops.len() < 2 {
        // Routes with only 1 stop are pretty much useless, and it makes border matching quite
        // confusing.
        return None;
    }

    Some(RawBusRoute {
        full_name,
        short_name,
        is_bus,
        osm_rel_id: rel.id,
        stops: keep_stops,
        border_start: None,
        border_end: None,
        all_pts,
    })
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

fn rel_url(id: i64) -> String {
    format!("https://www.openstreetmap.org/relation/{}", id)
}
