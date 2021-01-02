use abstutil::Timer;
use geom::{Distance, FindClosest, PolyLine};
use kml::ExtraShapes;
use map_model::osm;
use map_model::raw::{OriginalRoad, RawMap};

use crate::{OnstreetParking, Options, PrivateOffstreetParking, PublicOffstreetParking};

// Just used for matching hints to different sides of a road.
const DIRECTED_ROAD_THICKNESS: Distance = Distance::const_meters(2.5);

pub fn apply_parking(map: &mut RawMap, opts: &Options, timer: &mut Timer) {
    match opts.onstreet_parking {
        OnstreetParking::JustOSM => {}
        OnstreetParking::Blockface(ref path) => {
            use_parking_hints(map, path.clone(), timer);
        }
        OnstreetParking::SomeAdditionalWhereNoData { pct } => {
            let pct = pct as i64;
            for (id, r) in map.roads.iter_mut() {
                // The 20m minimum is a heuristic. PARKING_SPOT_LENGTH is only 8m, but we haven't
                // trimmed roads between intersections yet.
                if r.osm_tags.contains_key(osm::INFERRED_PARKING)
                    && r.osm_tags
                        .is_any(osm::HIGHWAY, vec!["residential", "tertiary"])
                    && !r.osm_tags.is("foot", "no")
                    && id.osm_way_id.0 % 100 <= pct
                    && PolyLine::unchecked_new(r.center_points.clone()).length()
                        >= Distance::meters(20.0)
                {
                    if r.osm_tags.is("oneway", "yes") {
                        r.osm_tags.remove(osm::PARKING_BOTH);
                        r.osm_tags.insert(osm::PARKING_RIGHT, "parallel");
                    } else {
                        r.osm_tags.insert(osm::PARKING_BOTH, "parallel");
                    }
                }
            }
        }
    }
    match opts.public_offstreet_parking {
        PublicOffstreetParking::None => {}
        PublicOffstreetParking::GIS(ref path) => {
            use_offstreet_parking(map, path.clone(), timer);
        }
    }
    apply_private_offstreet_parking(map, &opts.private_offstreet_parking);
}

fn use_parking_hints(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("apply parking hints");
    let shapes: ExtraShapes = abstio::read_binary(path, timer);

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(OriginalRoad, bool)> =
        FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, r) in &map.roads {
        if r.is_light_rail() || r.is_footway() || r.is_service() {
            continue;
        }
        let center = PolyLine::must_new(r.center_points.clone());
        closest.add(
            (*id, true),
            center.must_shift_right(DIRECTED_ROAD_THICKNESS).points(),
        );
        closest.add(
            (*id, false),
            center.must_shift_left(DIRECTED_ROAD_THICKNESS).points(),
        );
    }

    for s in shapes.shapes.into_iter() {
        let pts = map.gps_bounds.convert(&s.points);
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
            let tags = &mut map.roads.get_mut(&r).unwrap().osm_tags;

            // Skip if the road already has this mapped.
            if !tags.contains_key(osm::INFERRED_PARKING) {
                continue;
            }

            let category = s.attributes.get("PARKING_CATEGORY");
            let has_parking = category != Some(&"None".to_string())
                && category != Some(&"No Parking Allowed".to_string());

            let definitely_no_parking =
                tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link", "trunk"]);
            if has_parking && definitely_no_parking {
                timer.warn(format!(
                    "Blockface says there's parking along motorway {}, ignoring",
                    r
                ));
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

            if let Some(both) = tags.remove(osm::PARKING_BOTH) {
                tags.insert(osm::PARKING_LEFT, both.clone());
                tags.insert(osm::PARKING_RIGHT, both);
            }

            tags.insert(
                if fwds {
                    osm::PARKING_RIGHT
                } else {
                    osm::PARKING_LEFT
                },
                if has_parking {
                    "parallel"
                } else {
                    "no_parking"
                },
            );

            // Maybe fold back into "both"
            if tags.contains_key(osm::PARKING_LEFT)
                && tags.get(osm::PARKING_LEFT) == tags.get(osm::PARKING_RIGHT)
            {
                let value = tags.remove(osm::PARKING_LEFT).unwrap();
                tags.remove(osm::PARKING_RIGHT).unwrap();
                tags.insert(osm::PARKING_BOTH, value);
            }
        }
    }
    timer.stop("apply parking hints");
}

fn use_offstreet_parking(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("match offstreet parking points");
    let shapes: ExtraShapes = abstio::read_binary(path, timer);

    let mut closest: FindClosest<osm::OsmID> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest.add(*id, b.polygon.points());
    }

    // TODO Another function just to use ?. Try blocks would rock.
    let mut handle_shape: Box<dyn FnMut(kml::ExtraShape) -> Option<()>> = Box::new(|s| {
        assert_eq!(s.points.len(), 1);
        let pt = s.points[0].to_pt(&map.gps_bounds);
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
