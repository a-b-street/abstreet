use std::collections::{BTreeMap, BTreeSet, HashMap};

use osm::{NodeID, OsmID, RelationID, WayID};

use abstutil::{retain_btreemap, Tags, Timer};
use geom::{HashablePt2D, Polygon, Pt2D, Ring};
use kml::{ExtraShape, ExtraShapes};
use map_model::raw::{RawArea, RawBuilding, RawMap, RawParkingLot, RawRoad, RestrictionType};
use map_model::{osm, Amenity, AreaType, NamePerLanguage};

use crate::osm_geom::{get_multipolygon_members, glue_multipolygon, multipoly_geometry};
use crate::{transit, Options};

pub struct OsmExtract {
    /// Unsplit roads
    pub roads: Vec<(WayID, RawRoad)>,
    /// Traffic signals to the direction they apply (or just true if unspecified)
    pub traffic_signals: HashMap<HashablePt2D, bool>,
    pub osm_node_ids: HashMap<HashablePt2D, NodeID>,
    /// (ID, restriction type, from way ID, via node ID, to way ID)
    pub simple_turn_restrictions: Vec<(RestrictionType, WayID, NodeID, WayID)>,
    /// (relation ID, from way ID, via way ID, to way ID)
    pub complicated_turn_restrictions: Vec<(RelationID, WayID, WayID, WayID)>,
    /// (location, amenity)
    pub amenities: Vec<(Pt2D, Amenity)>,
}

pub fn extract_osm(map: &mut RawMap, opts: &Options, timer: &mut Timer) -> OsmExtract {
    let mut doc = crate::reader::read(&opts.osm_input, &map.gps_bounds, timer).unwrap();

    // Use this to quickly test overrides to some ways before upstreaming in OSM.
    if false {
        let ways: BTreeSet<WayID> = abstutil::read_json("osm_ways.json".to_string(), timer);
        for id in ways {
            doc.ways
                .get_mut(&id)
                .unwrap()
                .tags
                .insert("junction", "intersection");
        }
    }

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
        for amenity in get_bldg_amenities(&node.tags) {
            out.amenities.push((node.pt, amenity));
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
            map.parking_aisles.push((id, way.pts.clone()));

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

    if map.name.city != "oneshot"
        && ((map.name.city == "seattle" && map.name.map == "huge_seattle")
            || map.name.city != "seattle")
    {
        abstutil::write_binary(
            abstutil::path(format!("input/{}/footways.bin", map.name.city)),
            &extra_footways,
        );
        abstutil::write_binary(
            abstutil::path(format!("input/{}/service_roads.bin", map.name.city)),
            &extra_service_roads,
        );
    }

    let boundary = map.boundary_polygon.clone().into_ring();

    let mut amenity_areas: Vec<(Polygon, Amenity)> = Vec::new();
    // Vehicle position (stop) -> pedestrian position (platform)
    let mut stop_areas: Vec<((osm::NodeID, Pt2D), Pt2D)> = Vec::new();

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
                for polygon in glue_multipolygon(
                    id,
                    get_multipolygon_members(id, rel, &doc),
                    Some(&boundary),
                    timer,
                ) {
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
            for polygon in glue_multipolygon(
                id,
                get_multipolygon_members(id, rel, &doc),
                Some(&boundary),
                timer,
            ) {
                map.parking_lots.push(RawParkingLot {
                    osm_id: OsmID::Relation(id),
                    polygon,
                    osm_tags: rel.tags.clone(),
                });
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
                        amenity_areas.push((ring.to_polygon(), amenity.clone()));
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
                        stops.push((*n, pt));
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
    for polygon in glue_multipolygon(RelationID(-1), coastline_groups, Some(&boundary), timer) {
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
    for (poly, amenity) in amenity_areas {
        timer.next();
        for b in map.buildings.values_mut() {
            if poly.contains_pt(b.polygon.center()) {
                b.amenities.push(amenity.clone());
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
        "cycleway",
        "footway",
        "living_street",
        "motorway",
        "motorway_link",
        "path",
        "primary",
        "primary_link",
        "residential",
        "secondary",
        "secondary_link",
        "service",
        "steps",
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

    if (highway == "cycleway" || highway == "footway" || highway == "path" || highway == "steps")
        && opts.map_config.inferred_sidewalks
    {
        return false;
    }
    if (highway == "cycleway" || highway == "path")
        && !tags.is_any("foot", vec!["yes", "designated"])
    {
        return false;
    }

    // Service roads can represent lots of things, most of which we don't want to keep yet. What's
    // allowed here is just based on what's been encountered so far in Seattle and Kraków.
    if highway == "service" {
        let for_buses = tags.is_any("psv", vec!["bus", "yes"]) || tags.is("bus", "yes");
        if !for_buses && !tags.is("service", "alley") {
            return false;
        }
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
        if tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
            || tags.is_any("junction", vec!["intersection", "roundabout"])
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

fn get_bldg_amenities(tags: &Tags) -> Vec<Amenity> {
    let mut amenities = Vec::new();
    for key in vec!["amenity", "shop"] {
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
    if tags.is_any("leisure", vec!["park", "golf_course"]) {
        return Some(AreaType::Park);
    }
    if tags.is_any("natural", vec!["wood", "scrub"]) {
        return Some(AreaType::Park);
    }
    if tags.is_any("landuse", vec!["cemetery", "forest", "grass"])
        || tags.is("amenity", "graveyard")
    {
        return Some(AreaType::Park);
    }

    if tags.is("natural", "water") || tags.is("waterway", "riverbank") {
        return Some(AreaType::Water);
    }

    if tags.is("place", "island") {
        return Some(AreaType::Island);
    }

    None
}
