mod group_parcels;
mod neighborhoods;
mod osm;
mod remove_disconnected;
mod split_ways;
mod srtm;

use crate::srtm::Elevation;
use abstutil::Timer;
use geom::{Distance, GPSBounds, PolyLine, Pt2D};
use kml::ExtraShapes;
use map_model::{raw_data, FindClosest, IntersectionType, LANE_THICKNESS};
use std::path::Path;
use structopt::StructOpt;

const MAX_DIST_BTWN_INTERSECTION_AND_SIGNAL: Distance = Distance::const_meters(50.0);

#[derive(StructOpt, Debug)]
#[structopt(name = "convert_osm")]
pub struct Flags {
    /// OSM XML file to read
    #[structopt(long = "osm")]
    pub osm: String,

    /// HGT with elevation data
    #[structopt(long = "elevation")]
    pub elevation: String,

    /// KML with traffic signals
    #[structopt(long = "traffic_signals")]
    pub traffic_signals: String,

    /// ExtraShapes file with parcels, produced using the kml crate
    #[structopt(long = "parcels")]
    pub parcels: String,

    /// ExtraShapes file with blockface, produced using the kml crate
    #[structopt(long = "parking_shapes")]
    pub parking_shapes: String,

    /// GTFS directory
    #[structopt(long = "gtfs")]
    pub gtfs: String,

    /// Neighborhood GeoJSON path
    #[structopt(long = "neighborhoods")]
    pub neighborhoods: String,

    /// Output .abst path
    #[structopt(long = "output")]
    pub output: String,

    /// Disable parcels and blockface
    #[structopt(long = "fast_dev")]
    pub fast_dev: bool,
}

pub fn convert(flags: &Flags, timer: &mut abstutil::Timer) -> raw_data::Map {
    let elevation = Elevation::new(&flags.elevation).expect("loading .hgt failed");
    let mut map = split_ways::split_up_roads(osm::osm_to_raw_roads(&flags.osm, timer), &elevation);
    remove_disconnected::remove_disconnected_roads(&mut map, timer);
    let gps_bounds = map.get_gps_bounds();

    if flags.fast_dev {
        return map;
    }

    use_parking_hints(&mut map, &gps_bounds, &flags.parking_shapes, timer);
    handle_parcels(&mut map, &gps_bounds, &flags.parcels, timer);
    handle_traffic_signals(&mut map, &gps_bounds, &flags.traffic_signals, timer);
    map.bus_routes = gtfs::load(&flags.gtfs).unwrap();

    {
        let map_name = Path::new(&flags.output)
            .file_stem()
            .unwrap()
            .to_os_string()
            .into_string()
            .unwrap();
        neighborhoods::convert(&flags.neighborhoods, map_name, &gps_bounds);
    }

    map
}

fn use_parking_hints(
    map: &mut raw_data::Map,
    gps_bounds: &GPSBounds,
    path: &str,
    timer: &mut Timer,
) {
    println!("Loading blockface shapes from {}", path);
    let shapes: ExtraShapes = abstutil::read_binary(path, timer).expect("loading blockface failed");

    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(raw_data::StableRoadID, bool)> =
        FindClosest::new(&gps_bounds.to_bounds());
    for (id, r) in &map.roads {
        let pts = PolyLine::new(
            r.points
                .iter()
                .map(|pt| Pt2D::from_gps(*pt, gps_bounds).unwrap())
                .collect(),
        );

        closest.add((*id, true), &pts.shift_right(LANE_THICKNESS));
        closest.add((*id, false), &pts.shift_left(LANE_THICKNESS));
    }

    'SHAPE: for s in shapes.shapes.into_iter() {
        let mut pts: Vec<Pt2D> = Vec::new();
        for pt in s.points.into_iter() {
            if let Some(pt) = Pt2D::from_gps(pt, gps_bounds) {
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
}

fn handle_parcels(map: &mut raw_data::Map, gps_bounds: &GPSBounds, path: &str, timer: &mut Timer) {
    println!("Loading parcels from {}", path);
    let parcels: ExtraShapes = abstutil::read_binary(path, timer).expect("loading parcels failed");
    println!(
        "Finding matching parcels from {} candidates",
        parcels.shapes.len()
    );
    for p in parcels.shapes.into_iter() {
        if p.points.len() > 1
            && p.points
                .iter()
                .find(|pt| !gps_bounds.contains(**pt))
                .is_none()
        {
            map.parcels.push(raw_data::Parcel {
                points: p.points,
                block: 0,
            });
        }
    }
    group_parcels::group_parcels(gps_bounds, &mut map.parcels);
}

fn handle_traffic_signals(
    map: &mut raw_data::Map,
    gps_bounds: &GPSBounds,
    path: &str,
    timer: &mut Timer,
) {
    for shape in kml::load(path, gps_bounds, timer)
        .expect("loading traffic signals failed")
        .shapes
        .into_iter()
    {
        // See https://www.seattle.gov/Documents/Departments/SDOT/GIS/Traffic_Signals_OD.pdf
        if shape.points.len() > 1 {
            panic!("Traffic signal has multiple points: {:?}", shape);
        }
        let pt = shape.points[0];
        if gps_bounds.contains(pt) {
            // TODO use a quadtree or some better way to match signals to the closest
            // intersection
            let closest_intersection = map
                .intersections
                .values_mut()
                .min_by_key(|i| pt.gps_dist_meters(i.point))
                .unwrap();
            let dist = pt.gps_dist_meters(closest_intersection.point);
            if dist <= MAX_DIST_BTWN_INTERSECTION_AND_SIGNAL {
                if closest_intersection.intersection_type == IntersectionType::TrafficSignal {
                    println!("WARNING: {:?} already has a traffic signal, but there's another one that's {} from it", closest_intersection, dist);
                }
                closest_intersection.intersection_type = IntersectionType::TrafficSignal;
            }
        }
    }
}
