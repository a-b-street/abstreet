mod clip;
mod neighborhoods;
mod osm;
mod remove_disconnected;
mod split_ways;

use abstutil::Timer;
use geom::{Distance, FindClosest, GPSBounds, LonLat, PolyLine, Pt2D};
use kml::ExtraShapes;
use map_model::{raw_data, OffstreetParking, LANE_THICKNESS};
use std::fs::File;
use std::io::{BufRead, BufReader};
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

    /// KML file with offstreet parking info. Optional.
    #[structopt(long = "offstreet_parking", default_value = "")]
    pub offstreet_parking: String,

    /// GTFS directory. Optional.
    #[structopt(long = "gtfs", default_value = "")]
    pub gtfs: String,

    /// Neighborhood GeoJSON path. Optional.
    #[structopt(long = "neighborhoods", default_value = "")]
    pub neighborhoods: String,

    /// Osmosis clipping polgon. Required.
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
    if flags.clip.is_empty() {
        panic!("You must specify an Osmosis boundary polygon with --clip");
    }
    let boundary_polygon_pts = read_osmosis_polygon(&flags.clip);
    let mut gps_bounds = GPSBounds::new();
    for pt in &boundary_polygon_pts {
        gps_bounds.update(*pt);
    }

    let mut map = split_ways::split_up_roads(
        osm::osm_to_raw_roads(&flags.osm, &gps_bounds, timer),
        gps_bounds,
        timer,
    );
    clip::clip_map(&mut map, boundary_polygon_pts, timer);
    remove_disconnected::remove_disconnected_roads(&mut map, timer);

    if flags.fast_dev {
        return map;
    }

    if !flags.parking_shapes.is_empty() {
        use_parking_hints(&mut map, &flags.parking_shapes, timer);
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

fn use_parking_hints(map: &mut raw_data::Map, path: &str, timer: &mut Timer) {
    timer.start("apply parking hints");
    println!("Loading blockface shapes from {}", path);
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

    'SHAPE: for s in shapes.shapes.into_iter() {
        let mut pts: Vec<Pt2D> = Vec::new();
        for pt in s.points.into_iter() {
            if let Some(pt) = Pt2D::from_gps(pt, &map.gps_bounds) {
                pts.push(pt);
            } else {
                continue 'SHAPE;
            }
        }
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

fn read_osmosis_polygon(path: &str) -> Vec<LonLat> {
    let mut pts: Vec<LonLat> = Vec::new();
    for (idx, maybe_line) in BufReader::new(File::open(path).unwrap())
        .lines()
        .enumerate()
    {
        if idx == 0 || idx == 1 {
            continue;
        }
        let line = maybe_line.unwrap();
        if line == "END" {
            break;
        }
        let parts: Vec<&str> = line.trim_start().split("    ").collect();
        assert!(parts.len() == 2);
        let lon = parts[0].parse::<f64>().unwrap();
        let lat = parts[1].parse::<f64>().unwrap();
        pts.push(LonLat::new(lon, lat));
    }
    pts
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
        assert_eq!(map.buildings[idx].parking, None);
        map.buildings[idx].parking = Some(OffstreetParking { name, num_stalls });
        None
    });

    for s in shapes.shapes.into_iter() {
        handle_shape(s);
    }
    timer.stop("match offstreet parking points");
}
