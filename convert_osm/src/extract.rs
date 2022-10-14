use osm::{OsmID, RelationID, WayID};

use abstutil::{MultiMap, Tags, Timer};
use geom::{Distance, FindClosest, Polygon, Pt2D, Ring};
use osm2streets::{osm, NamePerLanguage};
use raw_map::{Amenity, AreaType, RawArea, RawBuilding, RawMap, RawParkingLot};

use crate::Options;
use streets_reader::osm_reader::{get_multipolygon_members, glue_multipolygon, multipoly_geometry};
use streets_reader::OsmExtract;

pub fn extract_osm(
    map: &mut RawMap,
    osm_input_path: &str,
    clip_path: Option<String>,
    opts: &Options,
    timer: &mut Timer,
) -> (OsmExtract, MultiMap<osm::WayID, String>) {
    let osm_xml = fs_err::read_to_string(osm_input_path).unwrap();
    let mut doc =
        streets_reader::osm_reader::read(&osm_xml, &map.streets.gps_bounds, timer).unwrap();

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
    if let Some(way) = doc.ways.get_mut(&WayID(332060260)) {
        way.tags.insert("sidewalk", "right");
    }
    if let Some(way) = doc.ways.get_mut(&WayID(332060236)) {
        way.tags.insert("sidewalk", "right");
    }

    if clip_path.is_none() {
        // Use the boundary from .osm.
        map.streets.gps_bounds = doc.gps_bounds.clone();
        map.streets.boundary_polygon = map.streets.gps_bounds.to_bounds().get_rectangle();
    }

    let mut out = OsmExtract::new();
    let mut amenity_points = Vec::new();
    let mut bus_routes_on_roads: MultiMap<osm::WayID, String> = MultiMap::new();

    timer.start_iter("processing OSM nodes", doc.nodes.len());
    for (id, node) in &doc.nodes {
        timer.next();
        out.handle_node(*id, node);
        for amenity in get_bldg_amenities(&node.tags) {
            amenity_points.push((node.pt, amenity));
        }
    }

    let mut coastline_groups: Vec<(WayID, Vec<Pt2D>)> = Vec::new();
    let mut memorial_areas: Vec<Polygon> = Vec::new();
    let mut amenity_areas: Vec<(Polygon, Amenity)> = Vec::new();
    timer.start_iter("processing OSM ways", doc.ways.len());
    for (id, way) in &mut doc.ways {
        timer.next();
        let id = *id;

        way.tags.insert(osm::OSM_WAY_ID, id.0.to_string());

        if out.handle_way(id, &way, opts) {
            continue;
        } else if way.tags.is(osm::HIGHWAY, "service") {
            // If we got here, is_road didn't interpret it as a normal road
            map.parking_aisles.push((id, way.pts.clone()));
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
        } else if way.tags.contains_key("amenity") {
            let amenity = Amenity {
                names: NamePerLanguage::new(&way.tags).unwrap_or_else(NamePerLanguage::unnamed),
                amenity_type: way.tags.get("amenity").unwrap().clone(),
                osm_tags: way.tags.clone(),
            };
            amenity_areas.push((polygon, amenity));
        }
    }

    let boundary = map.streets.boundary_polygon.get_outer_ring();

    // TODO Fill this out in a separate loop to keep a mutable borrow short. Maybe do this in
    // reader, or stop doing this entirely.
    for (id, rel) in &mut doc.relations {
        rel.tags.insert(osm::OSM_REL_ID, id.0.to_string());
    }

    timer.start_iter("processing OSM relations", doc.relations.len());
    for (id, rel) in &doc.relations {
        timer.next();
        let id = *id;

        if out.handle_relation(id, rel) {
            continue;
        } else if let Some(area_type) = get_area_type(&rel.tags) {
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
        } else if is_bldg(&rel.tags) {
            match multipoly_geometry(id, rel, &doc) {
                Ok(polygons) => {
                    for polygon in polygons {
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
                    // Sometimes the way is just the building, so we can directly update it
                    if let Some(b) = map.buildings.get_mut(member) {
                        b.amenities.push(amenity.clone());
                    } else if let Ok(ring) = Ring::new(doc.ways[w].pts.clone()) {
                        // Otherwise, match geometrically later on
                        amenity_areas.push((ring.into_polygon(), amenity.clone()));
                    }
                }
            }
        } else if rel.tags.is("type", "route") && rel.tags.is("route", "bus") {
            if let Some(name) = rel.tags.get("name") {
                for (role, member) in &rel.members {
                    if let OsmID::Way(w) = member {
                        if role.is_empty() {
                            bus_routes_on_roads.insert(*w, name.to_string());
                        }
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

    let mut closest_bldg: FindClosest<osm::OsmID> =
        FindClosest::new(&map.streets.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest_bldg.add_polygon(*id, &b.polygon);
    }

    timer.start_iter("match building amenities", amenity_points.len());
    for (pt, amenity) in amenity_points {
        timer.next();
        if let Some((id, _)) = closest_bldg.closest_pt(pt, Distance::meters(50.0)) {
            let b = map.buildings.get_mut(&id).unwrap();
            if b.polygon.contains_pt(pt) {
                b.amenities.push(amenity);
            }
        }
    }

    timer.start_iter("match buildings to amenity areas", amenity_areas.len());
    for (poly, amenity) in amenity_areas {
        timer.next();
        for b in closest_bldg.all_points_inside(&poly) {
            map.buildings
                .get_mut(&b)
                .unwrap()
                .amenities
                .push(amenity.clone());
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

    (out, bus_routes_on_roads)
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

    None
}

// Look for any service roads that collide with parking lots, and treat them as parking aisles
// instead.
fn find_parking_aisles(map: &mut RawMap, roads: &mut Vec<(WayID, Vec<Pt2D>, Tags)>) {
    let mut closest: FindClosest<usize> = FindClosest::new(&map.streets.gps_bounds.to_bounds());
    for (idx, lot) in map.parking_lots.iter().enumerate() {
        closest.add_polygon(idx, &lot.polygon);
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
