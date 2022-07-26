use osm::{OsmID, RelationID, WayID};

use abstio::{CityName, MapName};
use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, Polygon, Pt2D, Ring};
use kml::{ExtraShape, ExtraShapes};
use raw_map::{Amenity, AreaType, RawArea, RawBuilding, RawMap, RawParkingLot};
use street_network::{osm, NamePerLanguage};

use crate::Options;
use import_streets::osm_reader::{get_multipolygon_members, glue_multipolygon, multipoly_geometry};
use import_streets::OsmExtract;

pub fn extract_osm(
    map: &mut RawMap,
    osm_input_path: &str,
    clip_path: Option<String>,
    opts: &Options,
    timer: &mut Timer,
) -> (OsmExtract, Vec<(Pt2D, Amenity)>) {
    let osm_xml = fs_err::read_to_string(osm_input_path).unwrap();
    let mut doc =
        import_streets::osm_reader::read(&osm_xml, &map.streets.gps_bounds, timer).unwrap();

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
    // Hack for Geneva, which maps sidewalks as separate ways
    let infer_both_sidewalks_for_oneways = map.name.city == CityName::new("ch", "geneva");

    if clip_path.is_none() {
        // Use the boundary from .osm.
        map.streets.gps_bounds = doc.gps_bounds.clone();
        map.streets.boundary_polygon = map.streets.gps_bounds.to_bounds().get_rectangle();
    }

    let mut out = OsmExtract::new();
    let mut amenity_points = Vec::new();

    timer.start_iter("processing OSM nodes", doc.nodes.len());
    for (id, node) in &doc.nodes {
        timer.next();
        out.handle_node(*id, node);
        for amenity in get_bldg_amenities(&node.tags) {
            amenity_points.push((node.pt, amenity));
        }
    }

    // and cycleways
    let mut extra_footways = ExtraShapes { shapes: Vec::new() };

    let mut coastline_groups: Vec<(WayID, Vec<Pt2D>)> = Vec::new();
    let mut memorial_areas: Vec<Polygon> = Vec::new();
    let mut amenity_areas: Vec<(Polygon, Amenity)> = Vec::new();
    timer.start_iter("processing OSM ways", doc.ways.len());
    for (id, way) in &mut doc.ways {
        timer.next();
        let id = *id;

        way.tags.insert(osm::OSM_WAY_ID, id.0.to_string());

        if out.handle_way(id, &way, opts, infer_both_sidewalks_for_oneways) {
            continue;
        } else if way.tags.is(osm::HIGHWAY, "service") {
            // If we got here, is_road didn't interpret it as a normal road
            map.parking_aisles.push((id, way.pts.clone()));
        } else if way
            .tags
            .is_any(osm::HIGHWAY, vec!["cycleway", "footway", "path"])
        {
            extra_footways.shapes.push(ExtraShape {
                points: map.streets.gps_bounds.convert_back(&way.pts),
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
        } else if way.tags.contains_key("amenity") {
            let amenity = Amenity {
                names: NamePerLanguage::new(&way.tags).unwrap_or_else(NamePerLanguage::unnamed),
                amenity_type: way.tags.get("amenity").unwrap().clone(),
                osm_tags: way.tags.clone(),
            };
            amenity_areas.push((polygon, amenity));
        }
    }

    // Since we're not actively working on using footways, stop generating except in Seattle. In
    // the future, this should only happen for the largest or canonical map per city, but there's
    // no way to express that right now.
    if map.name == MapName::seattle("huge_seattle") {
        abstio::write_binary(map.name.city.input_path("footways.bin"), &extra_footways);
    }

    let boundary = map.streets.boundary_polygon.clone().into_ring();

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

    (out, amenity_points)
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
    let mut closest: FindClosest<usize> = FindClosest::new(&map.streets.gps_bounds.to_bounds());
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
