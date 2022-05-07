use std::collections::{HashMap, HashSet};

use osm::{NodeID, OsmID, RelationID, WayID};

use abstio::{CityName, MapName};
use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, HashablePt2D, Polygon, Pt2D, Ring};
use kml::{ExtraShape, ExtraShapes};
use raw_map::{
    osm, Amenity, AreaType, Direction, DrivingSide, NamePerLanguage, RawArea, RawBuilding, RawMap,
    RawParkingLot, RestrictionType,
};

use crate::osm_geom::{get_multipolygon_members, glue_multipolygon, multipoly_geometry};
use crate::Options;

pub struct OsmExtract {
    /// Unsplit roads. These aren't RawRoads yet, because they may not obey those invariants.
    pub roads: Vec<(WayID, Vec<Pt2D>, Tags)>,
    /// Traffic signals to the direction they apply
    pub traffic_signals: HashMap<HashablePt2D, Direction>,
    pub osm_node_ids: HashMap<HashablePt2D, NodeID>,
    /// (ID, restriction type, from way ID, via node ID, to way ID)
    pub simple_turn_restrictions: Vec<(RestrictionType, WayID, NodeID, WayID)>,
    /// (relation ID, from way ID, via way ID, to way ID)
    pub complicated_turn_restrictions: Vec<(RelationID, WayID, WayID, WayID)>,
    /// (location, amenity)
    pub amenities: Vec<(Pt2D, Amenity)>,
    /// Crosswalks located at these points, which should be on a RawRoad's center line
    pub crosswalks: HashSet<HashablePt2D>,
}

pub fn extract_osm(
    map: &mut RawMap,
    osm_input_path: &str,
    clip_path: Option<String>,
    opts: &Options,
    timer: &mut Timer,
) -> OsmExtract {
    let mut doc = crate::reader::read(osm_input_path, &map.gps_bounds, timer).unwrap();

    // TODO Hacks to override OSM data. There's no problem upstream, but we want to accomplish
    // various things for A/B Street.
    for id in [380902156, 380902155, 568612970] {
        if let Some(way) = doc.ways.get_mut(&WayID(id)) {
            // https://www.openstreetmap.org/way/380902156 and friends look like a separate
            // cycleway smushed into the Lake Washington / Madison junction
            way.tags.remove("bicycle");
        }
    }
    if let Some(way) = doc.ways.get_mut(&WayID(332355467)) {
        way.tags.insert("junction", "intersection");
    }

    if clip_path.is_none() {
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
        crosswalks: HashSet::new(),
    };

    timer.start_iter("processing OSM nodes", doc.nodes.len());
    for (id, node) in &doc.nodes {
        timer.next();
        out.osm_node_ids.insert(node.pt.to_hashable(), *id);

        if node.tags.is(osm::HIGHWAY, "traffic_signals") {
            let dir = if node.tags.is("traffic_signals:direction", "backward") {
                Direction::Back
            } else {
                Direction::Fwd
            };
            out.traffic_signals.insert(node.pt.to_hashable(), dir);
        }
        if node.tags.is(osm::HIGHWAY, "crossing") {
            out.crosswalks.insert(node.pt.to_hashable());
        }
        for amenity in get_bldg_amenities(&node.tags) {
            out.amenities.push((node.pt, amenity));
        }
    }

    // and cycleways
    let mut extra_footways = ExtraShapes { shapes: Vec::new() };

    let mut coastline_groups: Vec<(WayID, Vec<Pt2D>)> = Vec::new();
    let mut memorial_areas: Vec<Polygon> = Vec::new();
    timer.start_iter("processing OSM ways", doc.ways.len());
    for (id, way) in &mut doc.ways {
        timer.next();
        let id = *id;

        way.tags.insert(osm::OSM_WAY_ID, id.0.to_string());

        if is_road(&mut way.tags, opts, &map.name) {
            // TODO Hardcoding these overrides. OSM is correct, these don't have
            // sidewalks; there's a crosswalk mapped. But until we can snap sidewalks properly, do
            // this to prevent the sidewalks from being disconnected.
            if id == WayID(332060260) || id == WayID(332060236) {
                way.tags.insert(osm::SIDEWALK, "right");
            }

            out.roads.push((id, way.pts.clone(), way.tags.clone()));
            continue;
        } else if way.tags.is(osm::HIGHWAY, "service") {
            // If we got here, is_road didn't interpret it as a normal road
            map.parking_aisles.push((id, way.pts.clone()));
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
            ring.into_polygon()
        } else {
            continue;
        };

        if is_bldg(&way.tags) {
            map.buildings.insert(
                OsmID::Way(id),
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
            map.parking_lots.push(RawParkingLot {
                osm_id: OsmID::Way(id),
                polygon,
                osm_tags: way.tags.clone(),
            });
        } else if way.tags.is("historic", "memorial") {
            memorial_areas.push(polygon);
        }
    }

    // Since we're not actively working on using footways, stop generating except in Seattle. In
    // the future, this should only happen for the largest or canonical map per city, but there's
    // no way to express that right now.
    if map.name == MapName::seattle("huge_seattle") {
        abstio::write_binary(map.name.city.input_path("footways.bin"), &extra_footways);
    }

    let boundary = map.boundary_polygon.clone().into_ring();

    let mut amenity_areas: Vec<(Polygon, Amenity)> = Vec::new();

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
                    glue_multipolygon(id, get_multipolygon_members(id, rel, &doc), Some(&boundary))
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
                    OsmID::Relation(r) => {
                        warn!("{} contains {} as {}", id, r, role);
                    }
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
                            warn!(
                                "Weird complicated turn restriction \"{}\" from {} to {} via {}: \
                                 {}",
                                restriction, from, to, via, id
                            );
                        }
                    }
                }
            }
        } else if is_bldg(&rel.tags) {
            match multipoly_geometry(id, rel, &doc) {
                Ok(polygon) => {
                    map.buildings.insert(
                        OsmID::Relation(id),
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
        } else if rel.tags.is("amenity", "parking") {
            for polygon in
                glue_multipolygon(id, get_multipolygon_members(id, rel, &doc), Some(&boundary))
            {
                map.parking_lots.push(RawParkingLot {
                    osm_id: OsmID::Relation(id),
                    polygon,
                    osm_tags: rel.tags.clone(),
                });
            }
        } else if rel.tags.is("type", "multipolygon") && rel.tags.contains_key("amenity") {
            let amenity = Amenity {
                names: NamePerLanguage::new(&rel.tags).unwrap_or_else(NamePerLanguage::unnamed),
                amenity_type: rel.tags.get("amenity").unwrap().clone(),
                osm_tags: rel.tags.clone(),
            };
            for (role, member) in &rel.members {
                if role != "outer" {
                    continue;
                }
                if let OsmID::Way(w) = member {
                    if let Ok(ring) = Ring::new(doc.ways[w].pts.clone()) {
                        amenity_areas.push((ring.into_polygon(), amenity.clone()));
                    }
                }
            }
        }
    }

    // Special case the coastline.
    println!("{} ways of coastline", coastline_groups.len());
    for polygon in glue_multipolygon(RelationID(-1), coastline_groups, Some(&boundary)) {
        let mut osm_tags = Tags::empty();
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
        map.buildings
            .retain(|_, b| !area.contains_pt(b.polygon.center()));
    }

    timer.start_iter("match buildings to amenity areas", amenity_areas.len());
    for (poly, amenity) in amenity_areas {
        timer.next();
        for b in map.buildings.values_mut() {
            if poly.contains_pt(b.polygon.center()) {
                b.amenities.push(amenity.clone());
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

    timer.start("find service roads crossing parking lots");
    find_parking_aisles(map, &mut out.roads);
    timer.stop("find service roads crossing parking lots");

    out
}

fn is_road(tags: &mut Tags, opts: &Options, name: &MapName) -> bool {
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
        "cycleway",
        "footway",
        "living_street",
        "motorway",
        "motorway_link",
        "path",
        "pedestrian",
        "primary",
        "primary_link",
        "residential",
        "secondary",
        "secondary_link",
        "service",
        "steps",
        "tertiary",
        "tertiary_link",
        "track",
        "trunk",
        "trunk_link",
        "unclassified",
    ]
    .contains(&highway.as_ref())
    {
        return false;
    }

    if highway == "track" && tags.is("bicycle", "no") {
        return false;
    }

    #[allow(clippy::collapsible_if)] // better readability
    if (highway == "footway" || highway == "path" || highway == "steps")
        && opts.map_config.inferred_sidewalks
    {
        if !tags.is_any("bicycle", vec!["designated", "yes", "dismount"]) {
            return false;
        }
    }
    if highway == "pedestrian"
        && tags.is("bicycle", "dismount")
        && opts.map_config.inferred_sidewalks
    {
        return false;
    }

    // Import most service roads. Always ignore driveways, golf cart paths, and always reserve
    // parking_aisles for parking lots.
    if highway == "service" && tags.is_any("service", vec!["driveway", "parking_aisle"]) {
        // An exception -- keep driveways signed for bikes
        if !(tags.is("service", "driveway") && tags.is("bicycle", "designated")) {
            return false;
        }
    }
    if highway == "service" && tags.is("golf", "cartpath") {
        return false;
    }
    if highway == "service" && tags.is("access", "customers") {
        return false;
    }

    // Not sure what this means, found in Seoul.
    if tags.is("lanes", "0") {
        return false;
    }

    if opts.skip_local_roads && osm::RoadRank::from_highway(highway) == osm::RoadRank::Local {
        return false;
    }

    // It's a road! Now fill in some possibly missing data.

    // If there's no parking data in OSM already, then assume no parking and mark that it's
    // inferred.
    if !tags.contains_key(osm::PARKING_LEFT)
        && !tags.contains_key(osm::PARKING_RIGHT)
        && !tags.contains_key(osm::PARKING_BOTH)
        && !tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link", "service"])
        && !tags.is("junction", "roundabout")
    {
        tags.insert(osm::PARKING_BOTH, "no_parking");
        tags.insert(osm::INFERRED_PARKING, "true");
    }

    // If there's no sidewalk data in OSM already, then make an assumption and mark that
    // it's inferred.
    if !tags.contains_key(osm::SIDEWALK) && opts.map_config.inferred_sidewalks {
        tags.insert(osm::INFERRED_SIDEWALKS, "true");

        if tags.contains_key("sidewalk:left") || tags.contains_key("sidewalk:right") {
            // Attempt to mangle
            // https://wiki.openstreetmap.org/wiki/Key:sidewalk#Separately_mapped_sidewalks_on_only_one_side
            // into left/right/both. We have to make assumptions for missing values.
            let right = !tags.is("sidewalk:right", "no");
            let left = !tags.is("sidewalk:left", "no");
            let value = match (right, left) {
                (true, true) => "both",
                (true, false) => "right",
                (false, true) => "left",
                (false, false) => "none",
            };
            tags.insert(osm::SIDEWALK, value);
        } else if tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
            || tags.is_any("junction", vec!["intersection", "roundabout"])
            || tags.is("foot", "no")
            || tags.is(osm::HIGHWAY, "service")
            // TODO For now, not attempting shared walking/biking paths.
            || tags.is_any(osm::HIGHWAY, vec!["cycleway", "pedestrian", "track"])
        {
            tags.insert(osm::SIDEWALK, "none");
        } else if tags.is("oneway", "yes") {
            if opts.map_config.driving_side == DrivingSide::Right {
                tags.insert(osm::SIDEWALK, "right");
            } else {
                tags.insert(osm::SIDEWALK, "left");
            }
            if tags.is_any(osm::HIGHWAY, vec!["residential", "living_street"])
                && !tags.is("dual_carriageway", "yes")
            {
                tags.insert(osm::SIDEWALK, "both");
            }
            // Hack for Geneva, which maps sidewalks as separate ways
            if name.city == CityName::new("ch", "geneva") {
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

fn get_bldg_amenities(tags: &Tags) -> Vec<Amenity> {
    let mut amenities = Vec::new();
    for key in ["amenity", "shop", "craft", "office", "tourism", "leisure"] {
        if let Some(amenity) = tags.get(key) {
            amenities.push(Amenity {
                names: NamePerLanguage::new(tags).unwrap_or_else(NamePerLanguage::unnamed),
                amenity_type: amenity.clone(),
                osm_tags: tags.clone(),
            });
        }
    }
    amenities
}

fn get_area_type(tags: &Tags) -> Option<AreaType> {
    if tags.is_any("leisure", vec!["garden", "park", "golf_course"]) {
        return Some(AreaType::Park);
    }
    if tags.is_any("natural", vec!["wood", "scrub"]) {
        return Some(AreaType::Park);
    }
    if tags.is_any(
        "landuse",
        vec![
            "cemetery",
            "flowerbed",
            "forest",
            "grass",
            "meadow",
            "recreation_ground",
            "village_green",
        ],
    ) || tags.is("amenity", "graveyard")
    {
        return Some(AreaType::Park);
    }

    if tags.is("natural", "water") || tags.is("waterway", "riverbank") {
        return Some(AreaType::Water);
    }

    if tags.is("place", "island") {
        return Some(AreaType::Island);
    }

    if tags.is(osm::HIGHWAY, "pedestrian") {
        return Some(AreaType::PedestrianPlaza);
    }

    None
}

// Look for any service roads that collide with parking lots, and treat them as parking aisles
// instead.
fn find_parking_aisles(map: &mut RawMap, roads: &mut Vec<(WayID, Vec<Pt2D>, Tags)>) {
    let mut closest: FindClosest<usize> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (idx, lot) in map.parking_lots.iter().enumerate() {
        closest.add(idx, lot.polygon.points());
    }
    let mut keep_roads = Vec::new();
    let mut parking_aisles = Vec::new();
    for (id, pts, osm_tags) in roads.drain(..) {
        if !osm_tags.is(osm::HIGHWAY, "service") {
            keep_roads.push((id, pts, osm_tags));
            continue;
        }
        // TODO This code is repeated later in make/parking_lots.rs, but oh well.

        // Use the center of all the aisle points to match it to lots
        let candidates: Vec<usize> = closest
            .all_close_pts(Pt2D::center(&pts), Distance::meters(500.0))
            .into_iter()
            .map(|(idx, _, _)| idx)
            .collect();
        if service_road_crosses_parking_lot(map, &pts, candidates) {
            parking_aisles.push((id, pts));
        } else {
            keep_roads.push((id, pts, osm_tags));
        }
    }
    roads.extend(keep_roads);
    for (id, pts) in parking_aisles {
        map.parking_aisles.push((id, pts));
    }
}

fn service_road_crosses_parking_lot(map: &RawMap, pts: &[Pt2D], candidates: Vec<usize>) -> bool {
    if let Ok((polylines, rings)) = Ring::split_points(pts) {
        for pl in polylines {
            for idx in &candidates {
                if map.parking_lots[*idx].polygon.clip_polyline(&pl).is_some() {
                    return true;
                }
            }
        }
        for ring in rings {
            for idx in &candidates {
                if map.parking_lots[*idx].polygon.clip_ring(&ring).is_some() {
                    return true;
                }
            }
        }
    }
    false
}
