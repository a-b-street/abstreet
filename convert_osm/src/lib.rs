mod clip;
mod osm_reader;
mod split_ways;
mod srtm;

use abstutil::Timer;
use geom::{Distance, FindClosest, PolyLine, Pt2D};
use kml::ExtraShapes;
use map_model::osm;
use map_model::raw::{DrivingSide, OriginalBuilding, OriginalRoad, RawMap};

// Just used for matching hints to different sides of a road.
const DIRECTED_ROAD_THICKNESS: Distance = Distance::const_meters(2.5);

pub struct Options {
    pub osm_input: String,
    pub city_name: String,
    pub name: String,

    pub parking_shapes: Option<String>,
    pub public_offstreet_parking: Option<String>,
    pub private_offstreet_parking: PrivateOffstreetParking,
    pub sidewalks: Option<String>,
    pub gtfs: Option<String>,
    pub elevation: Option<String>,
    pub clip: Option<String>,
    pub drive_on_right: bool,
}

// If a building doesn't have anything from public_offstreet_parking, how many private spots should
// it have?
pub enum PrivateOffstreetParking {
    FixedPerBldg(usize),
    // TODO Based on the number of residents?
}

pub fn convert(opts: Options, timer: &mut abstutil::Timer) -> RawMap {
    let (mut map, amenities) = split_ways::split_up_roads(
        osm_reader::extract_osm(
            &opts.osm_input,
            &opts.clip,
            &opts.city_name,
            &opts.name,
            timer,
        ),
        timer,
    );
    clip::clip_map(&mut map, timer);
    map.driving_side = if opts.drive_on_right {
        DrivingSide::Right
    } else {
        DrivingSide::Left
    };

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when
    // doing the parking hint matching.
    abstutil::retain_btreemap(&mut map.roads, |r, _| r.i1 != r.i2);

    use_amenities(&mut map, amenities, timer);

    if let Some(ref path) = opts.parking_shapes {
        use_parking_hints(&mut map, path.clone(), timer);
    }
    if let Some(ref path) = opts.public_offstreet_parking {
        use_offstreet_parking(&mut map, path.clone(), timer);
    }
    apply_private_offstreet_parking(&mut map, opts.private_offstreet_parking);
    if let Some(ref path) = opts.sidewalks {
        use_sidewalk_hints(&mut map, path.clone(), timer);
    }
    if let Some(ref path) = opts.gtfs {
        timer.start("load GTFS");
        map.bus_routes = gtfs::load(path);
        timer.stop("load GTFS");
    }
    if let Some(ref path) = opts.elevation {
        use_elevation(&mut map, path, timer);
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
            map.driving_side
                .right_shift(center.clone(), DIRECTED_ROAD_THICKNESS)
                .get(timer)
                .points(),
        );
        closest.add(
            (*id, false),
            map.driving_side
                .left_shift(center, DIRECTED_ROAD_THICKNESS)
                .get(timer)
                .points(),
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
        if let Some(((r, fwds), _)) = closest.closest_pt(middle, DIRECTED_ROAD_THICKNESS * 5.0) {
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

fn use_offstreet_parking(map: &mut RawMap, path: String, timer: &mut Timer) {
    timer.start("match offstreet parking points");
    let shapes: ExtraShapes = abstutil::read_binary(path, timer);

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

fn apply_private_offstreet_parking(map: &mut RawMap, policy: PrivateOffstreetParking) {
    match policy {
        PrivateOffstreetParking::FixedPerBldg(n) => {
            for b in map.buildings.values_mut() {
                if b.public_garage_name.is_none() {
                    assert_eq!(b.num_parking_spots, 0);
                    b.num_parking_spots = n;
                }
            }
        }
    }
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
            map.driving_side
                .right_shift(center.clone(), DIRECTED_ROAD_THICKNESS)
                .get(timer)
                .points(),
        );
        closest.add(
            (*id, false),
            map.driving_side
                .left_shift(center, DIRECTED_ROAD_THICKNESS)
                .get(timer)
                .points(),
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
            if let Some(((r, fwds), _)) = closest.closest_pt(middle, DIRECTED_ROAD_THICKNESS * 5.0)
            {
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

fn use_amenities(map: &mut RawMap, amenities: Vec<(Pt2D, String, String)>, timer: &mut Timer) {
    let mut closest: FindClosest<OriginalBuilding> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest.add(*id, b.polygon.points());
    }

    timer.start_iter("match building amenities", amenities.len());
    for (pt, name, amenity) in amenities {
        timer.next();
        if let Some((id, _)) = closest.closest_pt(pt, Distance::meters(50.0)) {
            let b = map.buildings.get_mut(&id).unwrap();
            if b.polygon.contains_pt(pt) {
                b.amenities.insert((name, amenity));
            }
        }
    }
}

fn use_elevation(map: &mut RawMap, path: &str, timer: &mut Timer) {
    timer.start("apply elevation data to intersections");
    let elevation = srtm::Elevation::load(path).unwrap();
    for i in map.intersections.values_mut() {
        i.elevation = elevation.get(i.point.forcibly_to_gps(&map.gps_bounds));
    }
    timer.stop("apply elevation data to intersections");
}
