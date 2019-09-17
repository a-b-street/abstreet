mod clip;
mod neighborhoods;
mod osm;
mod remove_disconnected;
mod split_ways;

use abstutil::{prettyprint_usize, Timer};
use geom::{Distance, FindClosest, Line, PolyLine, Pt2D};
use kml::ExtraShapes;
use map_model::{raw_data, LaneID, OffstreetParking, Position, LANE_THICKNESS};
use std::collections::BTreeMap;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "convert_osm")]
pub struct Flags {
    /// OSM XML file to read
    #[structopt(long = "osm")]
    pub osm: String,

    /// ExtraShapes file with blockface, produced using the kml crate. Optional.
    #[structopt(long = "parking_shapes", default_value = "")]
    pub parking_shapes: String,

    /// ExtraShapes file with street signs, produced using the kml crate. Optional.
    #[structopt(long = "street_signs", default_value = "")]
    pub street_signs: String,

    /// KML file with offstreet parking info. Optional.
    #[structopt(long = "offstreet_parking", default_value = "")]
    pub offstreet_parking: String,

    /// GTFS directory. Optional.
    #[structopt(long = "gtfs", default_value = "")]
    pub gtfs: String,

    /// Neighborhood GeoJSON path. Optional.
    #[structopt(long = "neighborhoods", default_value = "")]
    pub neighborhoods: String,

    /// Osmosis clipping polgon. Optional.
    #[structopt(long = "clip", default_value = "")]
    pub clip: String,

    /// Output .bin path
    #[structopt(long = "output")]
    pub output: String,

    /// Disable blockface
    #[structopt(long = "fast_dev")]
    pub fast_dev: bool,
}

pub fn convert(flags: &Flags, timer: &mut abstutil::Timer) -> raw_data::Map {
    let mut map =
        split_ways::split_up_roads(osm::extract_osm(&flags.osm, &flags.clip, timer), timer);
    clip::clip_map(&mut map, timer);
    remove_disconnected::remove_disconnected_roads(&mut map, timer);
    check_orig_ids(&map);

    if flags.fast_dev {
        return map;
    }

    if !flags.parking_shapes.is_empty() {
        use_parking_hints(&mut map, &flags.parking_shapes, timer);
    }
    if !flags.street_signs.is_empty() {
        use_street_signs(&mut map, &flags.street_signs, timer);
    }
    if !flags.offstreet_parking.is_empty() {
        use_offstreet_parking(&mut map, &flags.offstreet_parking, timer);
    }
    if !flags.gtfs.is_empty() {
        timer.start("load GTFS");
        map.bus_routes = gtfs::load(&flags.gtfs).unwrap();
        timer.stop("load GTFS");
    }

    if !flags.neighborhoods.is_empty() {
        timer.start("convert neighborhood polygons");
        let map_name = abstutil::basename(&flags.output);
        neighborhoods::convert(&flags.neighborhoods, map_name, &map.gps_bounds);
        timer.stop("convert neighborhood polygons");
    }

    map
}

fn check_orig_ids(map: &raw_data::Map) {
    {
        let mut ids = BTreeMap::new();
        for (id, r) in &map.roads {
            if let Some(id2) = ids.get(&r.orig_id) {
                panic!(
                    "Both {} and {} have the same orig_id: {:?}",
                    id, id2, r.orig_id
                );
            } else {
                ids.insert(r.orig_id, *id);
            }
        }
    }

    {
        let mut ids = BTreeMap::new();
        for (id, i) in &map.intersections {
            if let Some(id2) = ids.get(&i.orig_id) {
                panic!(
                    "Both {} and {} have the same orig_id: {:?}",
                    id, id2, i.orig_id
                );
            } else {
                ids.insert(i.orig_id, *id);
            }
        }
    }
}

fn use_parking_hints(map: &mut raw_data::Map, path: &str, timer: &mut Timer) {
    timer.start("apply parking hints");
    let shapes: ExtraShapes = abstutil::read_binary(path, timer).expect("loading blockface failed");

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(raw_data::StableRoadID, bool)> =
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
        if pts.len() > 1 {
            // The blockface line endpoints will be close to other roads, so match based on the
            // middle of the blockface.
            // TODO Long blockfaces sometimes cover two roads. Should maybe find ALL matches within
            // the threshold distance?
            let middle = PolyLine::new(pts).middle();
            if let Some(((r, fwds), _)) = closest.closest_pt(middle, LANE_THICKNESS * 5.0) {
                let category = s.attributes.get("PARKING_CATEGORY");
                let has_parking = category != Some(&"None".to_string())
                    && category != Some(&"No Parking Allowed".to_string());
                // Blindly override prior values.
                if fwds {
                    map.roads.get_mut(&r).unwrap().parking_lane_fwd = has_parking;
                } else {
                    map.roads.get_mut(&r).unwrap().parking_lane_back = has_parking;
                }
            }
        }
    }
    timer.stop("apply parking hints");
}

fn use_street_signs(map: &mut raw_data::Map, path: &str, timer: &mut Timer) {
    timer.start("apply street signs to override parking hints");
    let shapes: ExtraShapes =
        abstutil::read_binary(path, timer).expect("loading street_signs failed");

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(raw_data::StableRoadID, bool)> =
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

    let mut applied = 0;
    for s in shapes.shapes.into_iter() {
        let pts = if let Some(pts) = map.gps_bounds.try_convert(&s.points) {
            pts
        } else {
            continue;
        };
        if pts.len() == 1 {
            if let Some(((r, fwds), _)) = closest.closest_pt(pts[0], LANE_THICKNESS * 5.0) {
                // TODO Model RPZ, paid on-street spots, limited times, etc.
                let no_parking =
                    s.attributes.get("TEXT") == Some(&"NO PARKING ANYTIME".to_string());
                if no_parking {
                    applied += 1;
                    if fwds {
                        map.roads.get_mut(&r).unwrap().parking_lane_fwd = false;
                    } else {
                        map.roads.get_mut(&r).unwrap().parking_lane_back = false;
                    }
                }
            }
        }
    }
    timer.note(format!(
        "Applied {} street signs",
        prettyprint_usize(applied)
    ));
    timer.stop("apply street signs to override parking hints");
}

fn use_offstreet_parking(map: &mut raw_data::Map, path: &str, timer: &mut Timer) {
    timer.start("match offstreet parking points");
    let shapes = kml::load(path, &map.gps_bounds, timer).expect("loading offstreet_parking failed");

    // Building indices
    let mut closest: FindClosest<usize> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (idx, b) in map.buildings.iter().enumerate() {
        closest.add(idx, b.polygon.points());
    }

    // TODO Another function just to use ?. Try blocks would rock.
    let mut handle_shape: Box<dyn FnMut(kml::ExtraShape) -> Option<()>> = Box::new(|s| {
        assert_eq!(s.points.len(), 1);
        let pt = Pt2D::from_gps(s.points[0], &map.gps_bounds)?;
        let (idx, _) = closest.closest_pt(pt, Distance::meters(50.0))?;
        // TODO Handle parking lots.
        if !map.buildings[idx].polygon.contains_pt(pt) {
            return None;
        }
        let name = s.attributes.get("DEA_FACILITY_NAME")?.to_string();
        let num_stalls = s.attributes.get("DEA_STALLS")?.parse::<usize>().ok()?;
        // TODO Update the existing one instead
        if let Some(ref existing) = map.buildings[idx].parking {
            // TODO Can't use timer inside this closure
            println!(
                "Two offstreet parking hints apply to building {}: {} @ {}, and {} @ {}",
                idx, existing.num_stalls, existing.name, num_stalls, name
            );
        }
        map.buildings[idx].parking = Some(OffstreetParking {
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
