mod clip;
mod neighborhoods;
mod osm_reader;
mod split_ways;

use abstutil::Timer;
use geom::{Distance, FindClosest, Line, PolyLine, Pt2D};
use kml::ExtraShapes;
use map_model::raw::{OriginalBuilding, OriginalRoad, RawMap};
use map_model::{osm, LaneID, OffstreetParking, Position, LANE_THICKNESS};

pub struct Flags {
    pub osm: String,
    pub parking_shapes: Option<String>,
    pub offstreet_parking: Option<String>,
    pub sidewalks: Option<String>,
    pub gtfs: Option<String>,
    pub neighborhoods: Option<String>,
    pub clip: Option<String>,
    pub output: String,
}

pub fn convert(flags: &Flags, timer: &mut abstutil::Timer) -> RawMap {
    let mut map = split_ways::split_up_roads(
        osm_reader::extract_osm(&flags.osm, &flags.clip, timer),
        timer,
    );
    clip::clip_map(&mut map, timer);

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when doing the parking hint matching.
    abstutil::retain_btreemap(&mut map.roads, |r, _| r.i1 != r.i2);

    if let Some(ref path) = flags.parking_shapes {
        use_parking_hints(&mut map, path.clone(), timer);
    }
    if let Some(ref path) = flags.offstreet_parking {
        use_offstreet_parking(&mut map, path, timer);
    }
    if let Some(ref path) = flags.sidewalks {
        use_sidewalk_hints(&mut map, path.clone(), timer);
    }
    if let Some(ref path) = flags.gtfs {
        timer.start("load GTFS");
        map.bus_routes = gtfs::load(path).unwrap();
        timer.stop("load GTFS");
    }

    if let Some(ref path) = flags.neighborhoods {
        timer.start("convert neighborhood polygons");
        neighborhoods::convert(path.clone(), map.name.clone(), &map.gps_bounds);
        timer.stop("convert neighborhood polygons");
    }

    map
}

fn use_parking_hints(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("apply parking hints");
    let shapes: ExtraShapes = abstutil::read_binary(path, timer);

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(OriginalRoad, bool)> =
        FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, r) in &map.roads {
        let center = PolyLine::new(r.center_points.clone());
        closest.add(
            (*id, true),
            center.shift_right(LANE_THICKNESS).get(timer).points(),
        );
        closest.add(
            (*id, false),
            center.shift_left(LANE_THICKNESS).get(timer).points(),
        );
    }

    for s in shapes.shapes.into_iter() {
        let pts = if let Some(pts) = map.gps_bounds.try_convert(&s.points) {
            pts
        } else {
            continue;
        };
        if pts.len() <= 1 {
            continue;
        }
        // The blockface line endpoints will be close to other roads, so match based on the
        // middle of the blockface.
        // TODO Long blockfaces sometimes cover two roads. Should maybe find ALL matches within
        // the threshold distance?
        let middle = if let Some(pl) = PolyLine::maybe_new(pts) {
            pl.middle()
        } else {
            // Weird blockface with duplicate points. Shrug.
            continue;
        };
        if let Some(((r, fwds), _)) = closest.closest_pt(middle, LANE_THICKNESS * 5.0) {
            let tags = &mut map.roads.get_mut(&r).unwrap().osm_tags;

            // Skip if the road already has this mapped.
            if !tags.contains_key(osm::INFERRED_PARKING) {
                continue;
            }

            let category = s.attributes.get("PARKING_CATEGORY");
            let has_parking = category != Some(&"None".to_string())
                && category != Some(&"No Parking Allowed".to_string());

            let definitely_no_parking = match tags.get(osm::HIGHWAY) {
                Some(hwy) => hwy == "motorway" || hwy == "motorway_link",
                None => false,
            };
            if has_parking && definitely_no_parking {
                timer.warn(format!(
                    "Blockface says there's parking along motorway {}, ignoring",
                    r
                ));
                continue;
            }

            if let Some(both) = tags.remove(osm::PARKING_BOTH) {
                tags.insert(osm::PARKING_LEFT.to_string(), both.clone());
                tags.insert(osm::PARKING_RIGHT.to_string(), both.clone());
            }

            tags.insert(
                if fwds {
                    osm::PARKING_RIGHT.to_string()
                } else {
                    osm::PARKING_LEFT.to_string()
                },
                if has_parking {
                    "parallel".to_string()
                } else {
                    "no_parking".to_string()
                },
            );

            // Maybe fold back into "both"
            if tags.contains_key(osm::PARKING_LEFT)
                && tags.get(osm::PARKING_LEFT) == tags.get(osm::PARKING_RIGHT)
            {
                let value = tags.remove(osm::PARKING_LEFT).unwrap();
                tags.remove(osm::PARKING_RIGHT).unwrap();
                tags.insert(osm::PARKING_BOTH.to_string(), value);
            }
        }
    }
    timer.stop("apply parking hints");
}

fn use_offstreet_parking(map: &mut RawMap, path: &str, timer: &mut Timer) {
    timer.start("match offstreet parking points");
    let shapes = kml::load(path, &map.gps_bounds, timer).expect("loading offstreet_parking failed");

    let mut closest: FindClosest<OriginalBuilding> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest.add(*id, b.polygon.points());
    }

    // TODO Another function just to use ?. Try blocks would rock.
    let mut handle_shape: Box<dyn FnMut(kml::ExtraShape) -> Option<()>> = Box::new(|s| {
        assert_eq!(s.points.len(), 1);
        let pt = Pt2D::from_gps(s.points[0], &map.gps_bounds)?;
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
        // TODO Update the existing one instead
        if let Some(ref existing) = map.buildings[&id].parking {
            // TODO Can't use timer inside this closure
            println!(
                "Two offstreet parking hints apply to {}: {} @ {}, and {} @ {}",
                id, existing.num_stalls, existing.name, num_stalls, name
            );
        }
        map.buildings.get_mut(&id).unwrap().parking = Some(OffstreetParking {
            name,
            num_stalls,
            // Temporary values, populate later
            driveway_line: Line::new(Pt2D::new(0.0, 0.0), Pt2D::new(1.0, 1.0)),
            driving_pos: Position::new(LaneID(0), Distance::ZERO),
        });
        None
    });

    for s in shapes.shapes.into_iter() {
        handle_shape(s);
    }
    timer.stop("match offstreet parking points");
}

fn use_sidewalk_hints(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("apply sidewalk hints");
    let shapes: ExtraShapes = abstutil::read_binary(path, timer);

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(OriginalRoad, bool)> =
        FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, r) in &map.roads {
        let center = PolyLine::new(r.center_points.clone());
        closest.add(
            (*id, true),
            center.shift_right(LANE_THICKNESS).get(timer).points(),
        );
        closest.add(
            (*id, false),
            center.shift_left(LANE_THICKNESS).get(timer).points(),
        );
    }

    for s in shapes.shapes.into_iter() {
        let pts = if let Some(pts) = map.gps_bounds.try_convert(&s.points) {
            pts
        } else {
            continue;
        };
        if pts.len() <= 1 {
            continue;
        }
        // The endpoints will be close to other roads, so match based on the middle of the
        // blockface.
        // TODO Long lines sometimes cover two roads. Should maybe find ALL matches within the
        // threshold distance?
        if let Some(middle) = PolyLine::maybe_new(pts).map(|pl| pl.middle()) {
            if let Some(((r, fwds), _)) = closest.closest_pt(middle, LANE_THICKNESS * 5.0) {
                let osm_tags = &mut map.roads.get_mut(&r).unwrap().osm_tags;

                // Skip if the road already has this mapped.
                if !osm_tags.contains_key(osm::INFERRED_SIDEWALKS) {
                    continue;
                }

                let definitely_no_sidewalks = match osm_tags.get(osm::HIGHWAY) {
                    Some(hwy) => hwy == "motorway" || hwy == "motorway_link",
                    None => false,
                };
                if definitely_no_sidewalks {
                    timer.warn(format!(
                        "Sidewalks shapefile says there's something along motorway {}, ignoring",
                        r
                    ));
                    continue;
                }

                if fwds {
                    if osm_tags.get(osm::SIDEWALK) == Some(&"left".to_string()) {
                        osm_tags.insert(osm::SIDEWALK.to_string(), "both".to_string());
                    } else {
                        osm_tags.insert(osm::SIDEWALK.to_string(), "right".to_string());
                    }
                } else {
                    if osm_tags.get(osm::SIDEWALK) == Some(&"right".to_string()) {
                        osm_tags.insert(osm::SIDEWALK.to_string(), "both".to_string());
                    } else {
                        osm_tags.insert(osm::SIDEWALK.to_string(), "left".to_string());
                    }
                }
            }
        }
    }
    timer.stop("apply sidewalk hints");
}
