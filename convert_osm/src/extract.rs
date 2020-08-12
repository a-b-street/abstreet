use crate::reader::{Document, Relation};
use crate::transit;
use crate::Options;
use abstutil::{retain_btreemap, Tags, Timer};
use geom::{HashablePt2D, PolyLine, Polygon, Pt2D, Ring};
use kml::{ExtraShape, ExtraShapes};
use map_model::raw::{
    OriginalBuilding, OriginalIntersection, RawArea, RawBuilding, RawMap, RawParkingLot, RawRoad,
    RestrictionType,
};
use map_model::{osm, AreaType};
use osm::{NodeID, OsmID, RelationID, WayID};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;

pub struct OsmExtract {
    // Unsplit roads
    pub roads: Vec<(WayID, RawRoad)>,
    // Traffic signals to the direction they apply (or just true if unspecified)
    pub traffic_signals: HashMap<HashablePt2D, bool>,
    pub osm_node_ids: HashMap<HashablePt2D, NodeID>,
    // (ID, restriction type, from way ID, via node ID, to way ID)
    pub simple_turn_restrictions: Vec<(RestrictionType, WayID, NodeID, WayID)>,
    // (relation ID, from way ID, via way ID, to way ID)
    pub complicated_turn_restrictions: Vec<(RelationID, WayID, WayID, WayID)>,
    // (location, name, amenity type)
    pub amenities: Vec<(Pt2D, String, String)>,
}

pub fn extract_osm(map: &mut RawMap, opts: &Options, timer: &mut Timer) -> OsmExtract {
    let mut doc = crate::reader::read(&opts.osm_input, &map.gps_bounds, timer).unwrap();
    if opts.clip.is_none() {
        // Use the boundary from .osm.
        map.gps_bounds = doc.gps_bounds.clone();
        map.boundary_polygon = map.gps_bounds.to_bounds().get_rectangle();
    }

    let mut out = OsmExtract {
        roads: Vec::new(),
        traffic_signals: HashMap::new(),
        osm_node_ids: HashMap::new(),
        simple_turn_restrictions: Vec::new(),
        complicated_turn_restrictions: Vec::new(),
        amenities: Vec::new(),
    };

    timer.start_iter("processing OSM nodes", doc.nodes.len());
    for (id, node) in &doc.nodes {
        timer.next();
        out.osm_node_ids.insert(node.pt.to_hashable(), *id);

        if node.tags.is(osm::HIGHWAY, "traffic_signals") {
            let backwards = node.tags.is("traffic_signals:direction", "backward");
            out.traffic_signals
                .insert(node.pt.to_hashable(), !backwards);
        }
        if let Some(amenity) = node.tags.get("amenity") {
            out.amenities.push((
                node.pt,
                node.tags
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "unnamed".to_string()),
                amenity.clone(),
            ));
        }
        if let Some(shop) = node.tags.get("shop") {
            out.amenities.push((
                node.pt,
                node.tags
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "unnamed".to_string()),
                shop.clone(),
            ));
        }
    }

    // and cycleways
    let mut extra_footways = ExtraShapes { shapes: Vec::new() };
    let mut extra_service_roads = ExtraShapes { shapes: Vec::new() };

    let mut coastline_groups: Vec<(WayID, Vec<Pt2D>)> = Vec::new();
    let mut memorial_areas: Vec<Polygon> = Vec::new();
    timer.start_iter("processing OSM ways", doc.ways.len());
    for (id, way) in &mut doc.ways {
        timer.next();
        let id = *id;

        way.tags.insert(osm::OSM_WAY_ID, id.0.to_string());

        if is_road(&mut way.tags, opts) {
            // TODO Hardcoding these overrides. OSM is correct, these don't have
            // sidewalks; there's a crosswalk mapped. But until we can snap sidewalks properly, do
            // this to prevent the sidewalks from being disconnected.
            if id == WayID(332060260) || id == WayID(332060236) {
                way.tags.insert(osm::SIDEWALK, "right");
            }

            out.roads.push((
                id,
                RawRoad {
                    center_points: way.pts.clone(),
                    osm_tags: way.tags.clone(),
                    turn_restrictions: Vec::new(),
                    complicated_turn_restrictions: Vec::new(),
                },
            ));
            continue;
        } else if way.tags.is(osm::HIGHWAY, "service") {
            // If we got here, is_road didn't interpret it as a normal road
            map.parking_aisles.push(way.pts.clone());

            extra_service_roads.shapes.push(ExtraShape {
                points: map.gps_bounds.convert_back(&way.pts),
                attributes: way.tags.inner().clone(),
            });
        } else if way
            .tags
            .is_any(osm::HIGHWAY, vec!["cycleway", "footway", "path"])
        {
            extra_footways.shapes.push(ExtraShape {
                points: map.gps_bounds.convert_back(&way.pts),
                attributes: way.tags.inner().clone(),
            });
        } else if way.tags.is("natural", "coastline") && !way.tags.is("place", "island") {
            coastline_groups.push((id, way.pts.clone()));
            continue;
        }

        // All the other cases we care about are areas.
        let mut deduped = way.pts.clone();
        deduped.dedup();
        let polygon = if let Ok(ring) = Ring::new(deduped) {
            ring.to_polygon()
        } else {
            continue;
        };

        if is_bldg(&way.tags) {
            map.buildings.insert(
                OriginalBuilding {
                    osm_id: OsmID::Way(id),
                },
                RawBuilding {
                    polygon,
                    public_garage_name: None,
                    num_parking_spots: 0,
                    amenities: get_bldg_amenities(&way.tags),
                    osm_tags: way.tags.clone(),
                },
            );
        } else if let Some(at) = get_area_type(&way.tags) {
            map.areas.push(RawArea {
                area_type: at,
                osm_id: OsmID::Way(id),
                polygon,
                osm_tags: way.tags.clone(),
            });
        } else if way.tags.is("amenity", "parking") {
            // TODO Verify parking = surface or handle other cases?
            map.parking_lots.push(RawParkingLot {
                polygon,
                osm_id: OsmID::Way(id),
            });
        } else if way.tags.is("historic", "memorial") {
            memorial_areas.push(polygon);
        }
    }

    if map.city_name != "oneshot"
        && ((map.city_name == "seattle" && map.name == "huge_seattle")
            || map.city_name != "seattle")
    {
        abstutil::write_binary(
            abstutil::path(format!("input/{}/footways.bin", map.city_name)),
            &extra_footways,
        );
        abstutil::write_binary(
            abstutil::path(format!("input/{}/service_roads.bin", map.city_name)),
            &extra_service_roads,
        );
    }

    let boundary = map.boundary_polygon.clone().into_ring();

    let mut amenity_areas: Vec<(String, String, Polygon)> = Vec::new();
    // Vehicle position (stop) -> pedestrian position (platform)
    let mut stop_areas: Vec<((OriginalIntersection, Pt2D), Pt2D)> = Vec::new();

    // TODO Fill this out in a separate loop to keep a mutable borrow short. Maybe do this in
    // reader, or stop doing this entirely.
    for (id, rel) in &mut doc.relations {
        rel.tags.insert(osm::OSM_REL_ID, id.0.to_string());
    }

    timer.start_iter("processing OSM relations", doc.relations.len());
    for (id, rel) in &doc.relations {
        timer.next();
        let id = *id;

        if let Some(area_type) = get_area_type(&rel.tags) {
            if rel.tags.is("type", "multipolygon") {
                for polygon in
                    glue_multipolygon(id, get_multipolygon_members(id, rel, &doc), &boundary)
                {
                    map.areas.push(RawArea {
                        area_type,
                        osm_id: OsmID::Relation(id),
                        polygon,
                        osm_tags: rel.tags.clone(),
                    });
                }
            }
        } else if rel.tags.is("type", "restriction") {
            let mut from_way_id: Option<WayID> = None;
            let mut via_node_id: Option<NodeID> = None;
            let mut via_way_id: Option<WayID> = None;
            let mut to_way_id: Option<WayID> = None;
            for (role, member) in &rel.members {
                match member {
                    OsmID::Way(w) => {
                        if role == "from" {
                            from_way_id = Some(*w);
                        } else if role == "to" {
                            to_way_id = Some(*w);
                        } else if role == "via" {
                            via_way_id = Some(*w);
                        }
                    }
                    OsmID::Node(n) => {
                        if role == "via" {
                            via_node_id = Some(*n);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            if let Some(restriction) = rel.tags.get("restriction") {
                if let Some(rt) = RestrictionType::new(restriction) {
                    if let (Some(from), Some(via), Some(to)) = (from_way_id, via_node_id, to_way_id)
                    {
                        out.simple_turn_restrictions.push((rt, from, via, to));
                    } else if let (Some(from), Some(via), Some(to)) =
                        (from_way_id, via_way_id, to_way_id)
                    {
                        if rt == RestrictionType::BanTurns {
                            out.complicated_turn_restrictions.push((id, from, via, to));
                        } else {
                            timer.warn(format!(
                                "Weird complicated turn restriction \"{}\" from {} to {} via {}: \
                                 {}",
                                restriction, from, to, via, id
                            ));
                        }
                    }
                }
            }
        } else if is_bldg(&rel.tags) {
            match multipoly_geometry(id, rel, &doc) {
                Ok(polygon) => {
                    map.buildings.insert(
                        OriginalBuilding {
                            osm_id: OsmID::Relation(id),
                        },
                        RawBuilding {
                            polygon,
                            public_garage_name: None,
                            num_parking_spots: 0,
                            amenities: get_bldg_amenities(&rel.tags),
                            osm_tags: rel.tags.clone(),
                        },
                    );
                }
                Err(err) => println!("Skipping building {}: {}", id, err),
            }
        } else if rel.tags.is("type", "route") {
            map.bus_routes.extend(transit::extract_route(
                id,
                rel,
                &doc,
                &map.boundary_polygon,
                timer,
            ));
        } else if rel.tags.is("type", "multipolygon") && rel.tags.contains_key("amenity") {
            let name = rel
                .tags
                .get("name")
                .cloned()
                .unwrap_or_else(|| "unnamed".to_string());
            let amenity = rel.tags.get("amenity").clone().unwrap();
            for (role, member) in &rel.members {
                if role != "outer" {
                    continue;
                }
                if let OsmID::Way(w) = member {
                    if let Ok(ring) = Ring::new(doc.ways[w].pts.clone()) {
                        amenity_areas.push((name.clone(), amenity.clone(), ring.to_polygon()));
                    }
                }
            }
        } else if rel.tags.is("public_transport", "stop_area") {
            let mut stops = Vec::new();
            let mut platform: Option<Pt2D> = None;
            for (role, member) in &rel.members {
                if let OsmID::Node(n) = member {
                    let pt = doc.nodes[n].pt;
                    if role == "stop" {
                        stops.push((OriginalIntersection { osm_node_id: *n }, pt));
                    } else if role == "platform" {
                        platform = Some(pt);
                    }
                } else if let OsmID::Way(w) = member {
                    if role == "platform" {
                        platform = Some(Pt2D::center(&doc.ways[w].pts));
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
    for polygon in glue_multipolygon(RelationID(-1), coastline_groups, &boundary) {
        let mut osm_tags = Tags::new(BTreeMap::new());
        osm_tags.insert("water", "ocean");
        // Put it at the beginning, so that it's naturally beneath island areas
        map.areas.insert(
            0,
            RawArea {
                area_type: AreaType::Water,
                osm_id: OsmID::Relation(RelationID(-1)),
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
        timer.next();
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

    out
}

fn is_road(tags: &mut Tags, opts: &Options) -> bool {
    if tags.is("area", "yes") {
        return false;
    }

    // First deal with railways.
    if tags.is("railway", "light_rail") {
        return true;
    }
    if tags.is("railway", "rail") && opts.include_railroads {
        return true;
    }
    // Explicitly need this to avoid overlapping geometry in Berlin.
    if tags.is("railway", "tram") {
        return false;
    }

    let highway = if let Some(x) = tags.get(osm::HIGHWAY) {
        if x == "construction" {
            // What exactly is under construction?
            if let Some(x) = tags.get("construction") {
                x
            } else {
                return false;
            }
        } else {
            x
        }
    } else {
        return false;
    };

    if !vec![
        "living_street",
        "motorway",
        "motorway_link",
        "primary",
        "primary_link",
        "residential",
        "secondary",
        "secondary_link",
        "service",
        "tertiary",
        "tertiary_link",
        "trunk",
        "trunk_link",
        "unclassified",
    ]
    .contains(&highway.as_ref())
    {
        return false;
    }

    // Service roads can represent lots of things, most of which we don't want to keep yet. What's
    // allowed here is just based on what's been encountered so far in Seattle and Kraków.
    if highway == "service" && !tags.is_any("psv", vec!["yes", "bus"]) && !tags.is("bus", "yes") {
        return false;
    }

    // Not sure what this means, found in Seoul.
    if tags.is("lanes", "0") {
        return false;
    }

    // It's a road! Now fill in some possibly missing data.

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
            || tags.is("foot", "no")
            || tags.is(osm::HIGHWAY, "service")
        {
            tags.insert(osm::SIDEWALK, "none");
        } else if tags.is("oneway", "yes") {
            tags.insert(osm::SIDEWALK, "right");
            if tags.is_any(osm::HIGHWAY, vec!["residential", "living_street"])
                && !tags.is("dual_carriageway", "yes")
            {
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
    if tags.is_any("natural", vec!["wood", "scrub"]) {
        return Some(AreaType::Park);
    }
    if tags.is_any("landuse", vec!["cemetery", "grass"]) || tags.is("amenity", "graveyard") {
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
        if tags.is(osm::HIGHWAY, "pedestrian") && tags.is("area", "yes") {
            return Some(AreaType::PedestrianIsland);
        }
    }

    None
}

fn get_multipolygon_members(
    id: RelationID,
    rel: &Relation,
    doc: &Document,
) -> Vec<(WayID, Vec<Pt2D>)> {
    let mut pts_per_way = Vec::new();
    for (role, member) in &rel.members {
        if let OsmID::Way(w) = member {
            if role == "outer" {
                pts_per_way.push((*w, doc.ways[w].pts.clone()));
            } else {
                println!("{} has unhandled member role {}, ignoring it", id, role);
            }
        }
    }
    pts_per_way
}

// The result could be more than one disjoint polygon.
fn glue_multipolygon(
    rel_id: RelationID,
    mut pts_per_way: Vec<(WayID, Vec<Pt2D>)>,
    boundary: &Ring,
) -> Vec<Polygon> {
    // First deal with all of the closed loops.
    let mut polygons: Vec<Polygon> = Vec::new();
    pts_per_way.retain(|(_, pts)| {
        if let Ok(ring) = Ring::new(pts.clone()) {
            polygons.push(ring.to_polygon());
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
                    "Throwing away {} chunks from relation {}: {:?}",
                    pts_per_way.len(),
                    rel_id,
                    pts_per_way.iter().map(|(id, _)| *id).collect::<Vec<_>>()
                );
                break;
            } else {
                reversed = true;
                result.reverse();
                // Try again!
            }
        }
    }

    result.dedup();
    if let Ok(ring) = Ring::new(result.clone()) {
        polygons.push(ring.to_polygon());
        return polygons;
    }
    if result.len() < 2 {
        return Vec::new();
    }
    if let Some(poly) = glue_to_boundary(PolyLine::must_new(result.clone()), boundary) {
        polygons.push(poly);
    } else {
        // Give up and just connect the ends directly.
        result.push(result[0]);
        polygons.push(Ring::must_new(result).to_polygon());
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
        trimmed_pts.extend(boundary_glue.into_points());
    } else {
        assert_eq!(trimmed_result.last_pt(), boundary_glue.last_pt());
        trimmed_pts.pop();
        trimmed_pts.extend(boundary_glue.reversed().into_points());
    }
    Some(Ring::must_new(trimmed_pts).to_polygon())
}

fn multipoly_geometry(
    rel_id: RelationID,
    rel: &Relation,
    doc: &Document,
) -> Result<Polygon, Box<dyn Error>> {
    let mut outer: Vec<Vec<Pt2D>> = Vec::new();
    let mut inner: Vec<Vec<Pt2D>> = Vec::new();
    for (role, member) in &rel.members {
        if let OsmID::Way(w) = member {
            let mut deduped = doc.ways[w].pts.clone();
            deduped.dedup();
            if deduped.len() < 3 {
                continue;
            }

            if role == "outer" {
                outer.push(deduped);
            } else if role == "inner" {
                inner.push(deduped);
            } else {
                return Err(format!("What's role {} for multipolygon {}?", role, rel_id).into());
            }
        }
    }
    // TODO Handle multiple outers with holes
    if outer.len() == 0 || outer.len() > 1 && !inner.is_empty() {
        return Err(format!(
            "Multipolygon {} has {} outer, {} inner. Huh?",
            rel_id,
            outer.len(),
            inner.len()
        )
        .into());
    }
    if inner.is_empty() {
        if outer.len() > 1 {
            Ok(Polygon::union_all(
                outer.into_iter().map(Polygon::buggy_new).collect(),
            ))
        } else {
            Ok(Polygon::buggy_new(outer.remove(0)))
        }
    } else {
        let mut inner_rings = Vec::new();
        for pts in inner {
            inner_rings.push(Ring::new(pts)?);
        }
        Ok(Polygon::with_holes(
            Ring::new(outer.pop().unwrap())?,
            inner_rings,
        ))
    }
}
