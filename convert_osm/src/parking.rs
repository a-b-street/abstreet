use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, PolyLine};
use kml::ExtraShapes;
use osm2streets::{osm, RoadID};
use raw_map::RawMap;

use crate::{OnstreetParking, Options, PrivateOffstreetParking, PublicOffstreetParking};

// Just used for matching hints to different sides of a road.
const DIRECTED_ROAD_THICKNESS: Distance = Distance::const_meters(2.5);

pub fn apply_parking(map: &mut RawMap, opts: &Options, timer: &mut Timer) {
    match opts.onstreet_parking {
        OnstreetParking::JustOSM => {}
        OnstreetParking::Blockface(ref path) => {
            use_parking_hints(map, path.clone(), timer);
        }
    }
    match opts.public_offstreet_parking {
        PublicOffstreetParking::None => {}
        PublicOffstreetParking::Gis(ref path) => {
            use_offstreet_parking(map, path.clone(), timer);
        }
    }
    apply_private_offstreet_parking(map, &opts.private_offstreet_parking);
}

fn unknown_parking(tags: &Tags) -> bool {
    !tags.contains_key("parking:lane:left")
        && !tags.contains_key("parking:lane:right")
        && !tags.contains_key("parking:lane:both")
        && !tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link", "service"])
        && !tags.is("junction", "roundabout")
}

fn use_parking_hints(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("apply parking hints");
    let shapes: ExtraShapes = abstio::read_binary(path, timer);

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(RoadID, bool)> =
        FindClosest::new(&map.streets.gps_bounds.to_bounds());
    for (id, r) in &map.streets.roads {
        if r.is_service() || !r.is_driveable() {
            continue;
        }
        closest.add(
            (*id, true),
            r.reference_line
                .must_shift_right(DIRECTED_ROAD_THICKNESS)
                .points(),
        );
        closest.add(
            (*id, false),
            r.reference_line
                .must_shift_left(DIRECTED_ROAD_THICKNESS)
                .points(),
        );
    }

    for s in shapes.shapes.into_iter() {
        let pts = map.streets.gps_bounds.convert(&s.points);
        if pts.len() <= 1 {
            continue;
        }
        // The blockface line endpoints will be close to other roads, so match based on the
        // middle of the blockface.
        // TODO Long blockfaces sometimes cover two roads. Should maybe find ALL matches within
        // the threshold distance?
        let middle = if let Ok(pl) = PolyLine::new(pts) {
            pl.middle()
        } else {
            // Weird blockface with duplicate points. Shrug.
            continue;
        };
        if let Some(((r, fwds), _)) = closest.closest_pt(middle, DIRECTED_ROAD_THICKNESS * 5.0) {
            let mut tags = map.road_to_osm_tags(r).cloned().unwrap_or_else(Tags::empty);

            // Skip if the road already has this mapped.
            if !unknown_parking(&tags) {
                continue;
            }

            let category = s.attributes.get("PARKING_CATEGORY");
            let has_parking = category != Some(&"None".to_string())
                && category != Some(&"No Parking Allowed".to_string());

            let definitely_no_parking =
                tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link", "trunk"]);
            if has_parking && definitely_no_parking {
                warn!(
                    "Blockface says there's parking along motorway {}, ignoring",
                    r
                );
                continue;
            }

            // Let's assume there isn't parking on the inner part of a dual carriageway
            if !fwds && tags.is("dual_carriageway", "yes") {
                continue;
            }
            // And definitely no parking in the middle of an intersection
            if tags.is("junction", "intersection") {
                continue;
            }

            if let Some(both) = tags.remove("parking:lane:both") {
                tags.insert("parking:lane:left", both.clone());
                tags.insert("parking:lane:right", both);
            }

            tags.insert(
                if fwds {
                    "parking:lane:right"
                } else {
                    "parking:lane:left"
                },
                if has_parking {
                    "parallel"
                } else {
                    "no_parking"
                },
            );

            // Maybe fold back into "both"
            if tags.contains_key("parking:lane:left")
                && tags.get("parking:lane:left") == tags.get("parking:lane:right")
            {
                let value = tags.remove("parking:lane:left").unwrap();
                tags.remove("parking:lane:right").unwrap();
                tags.insert("parking:lane:both", value);
            }

            // Remember that this isn't OSM data
            tags.insert("abst:parking_source", "blockface");

            let lane_specs_ltr = osm2streets::get_lane_specs_ltr(&tags, &map.streets.config);
            map.streets.roads.get_mut(&r).unwrap().lane_specs_ltr = lane_specs_ltr;

            // Note the change to the tag isn't saved, so regenerating lanes from tags later would
            // lose this
        }
    }
    timer.stop("apply parking hints");
}

fn use_offstreet_parking(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("match offstreet parking points");
    let shapes: ExtraShapes = abstio::read_binary(path, timer);

    let mut closest: FindClosest<osm::OsmID> =
        FindClosest::new(&map.streets.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest.add_polygon(*id, &b.polygon);
    }

    // TODO Another function just to use ?. Try blocks would rock.
    let mut handle_shape: Box<dyn FnMut(kml::ExtraShape) -> Option<()>> = Box::new(|s| {
        assert_eq!(s.points.len(), 1);
        let pt = s.points[0].to_pt(&map.streets.gps_bounds);
        let (id, _) = closest.closest_pt(pt, Distance::meters(50.0))?;
        // TODO Handle parking lots.
        if !map.buildings[&id].polygon.contains_pt(pt) {
            return None;
        }
        let name = s.attributes.get("DEA_FACILITY_NAME")?.to_string();
        let num_stalls = s.attributes.get("DEA_STALLS")?.parse::<usize>().ok()?;
        // Well that's silly. Why's it listed?
        if num_stalls == 0 {
            return None;
        }

        let bldg = map.buildings.get_mut(&id).unwrap();
        if bldg.num_parking_spots > 0 {
            // TODO Can't use timer inside this closure
            let old_name = bldg.public_garage_name.take().unwrap();
            println!(
                "Two offstreet parking hints apply to {}: {} @ {}, and {} @ {}",
                id, bldg.num_parking_spots, old_name, num_stalls, name
            );
            bldg.public_garage_name = Some(format!("{} and {}", old_name, name));
            bldg.num_parking_spots += num_stalls;
        } else {
            bldg.public_garage_name = Some(name);
            bldg.num_parking_spots = num_stalls;
        }
        None
    });

    for s in shapes.shapes.into_iter() {
        handle_shape(s);
    }
    timer.stop("match offstreet parking points");
}

fn apply_private_offstreet_parking(map: &mut RawMap, policy: &PrivateOffstreetParking) {
    match policy {
        PrivateOffstreetParking::FixedPerBldg(n) => {
            for b in map.buildings.values_mut() {
                if b.public_garage_name.is_none() {
                    assert_eq!(b.num_parking_spots, 0);

                    // Is it a parking garage?
                    if b.osm_tags.is("building", "parking") || b.osm_tags.is("amenity", "parking") {
                        let levels = b
                            .osm_tags
                            .get("parking:levels")
                            .or_else(|| b.osm_tags.get("building:levels"))
                            .and_then(|x| x.parse::<usize>().ok())
                            .unwrap_or(1);
                        // For multi-story garages, assume every floor has the same capacity. Guess
                        // 1 spot per 30m^2.
                        b.num_parking_spots = ((b.polygon.area() / 30.0) as usize) * levels;
                        // Not useful to list this
                        b.amenities.retain(|a| a.amenity_type != "parking");
                    } else {
                        b.num_parking_spots = *n;
                    }
                }
            }
        }
    }
}
