mod group_parcels;
mod neighborhoods;
mod osm;
mod remove_disconnected;
mod split_ways;
mod srtm;

use crate::srtm::Elevation;
use dimensioned::si;
use geom::{GPSBounds, PolyLine, Pt2D};
use kml::ExtraShapes;
use map_model::{raw_data, FindClosest, IntersectionType, LANE_THICKNESS};
use ordered_float::NotNan;
use std::path::Path;
use structopt::StructOpt;

const MAX_METERS_BTWN_INTERSECTION_AND_SIGNAL: f64 = 50.0;

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
}

pub fn convert(flags: &Flags, timer: &mut abstutil::Timer) -> raw_data::Map {
    let elevation = Elevation::new(&flags.elevation).expect("loading .hgt failed");
    let raw_map = osm::osm_to_raw_roads(&flags.osm, timer);
    let mut map = split_ways::split_up_roads(&raw_map, &elevation);
    remove_disconnected::remove_disconnected_roads(&mut map, timer);
    let gps_bounds = map.get_gps_bounds();

    println!("Loading blockface shapes from {}", flags.parking_shapes);
    let parking_shapes: ExtraShapes =
        abstutil::read_binary(&flags.parking_shapes, timer).expect("loading blockface failed");
    use_parking_hints(&mut map, parking_shapes, &gps_bounds);

    println!("Loading parcels from {}", flags.parcels);
    let parcels: ExtraShapes =
        abstutil::read_binary(&flags.parcels, timer).expect("loading parcels failed");
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
    group_parcels::group_parcels(&gps_bounds, &mut map.parcels);

    for shape in kml::load(&flags.traffic_signals, &gps_bounds, timer)
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
            let distance = |i: &raw_data::Intersection| pt.gps_dist_meters(i.point);

            // TODO use a quadtree or some better way to match signals to the closest
            // intersection
            let closest_intersection = map
                .intersections
                .iter_mut()
                .min_by_key(|i| NotNan::new(distance(i)).unwrap())
                .unwrap();
            let dist = distance(closest_intersection);
            if dist <= MAX_METERS_BTWN_INTERSECTION_AND_SIGNAL {
                if closest_intersection.intersection_type == IntersectionType::TrafficSignal {
                    println!("WARNING: {:?} already has a traffic signal, but there's another one that's {} from it", closest_intersection, dist);
                }
                closest_intersection.intersection_type = IntersectionType::TrafficSignal;
            }
        }
    }

    map.bus_routes = gtfs::load(&flags.gtfs).unwrap();

    let map_name = Path::new(&flags.output)
        .file_stem()
        .unwrap()
        .to_os_string()
        .into_string()
        .unwrap();
    neighborhoods::convert(&flags.neighborhoods, map_name, &gps_bounds);

    map
}

fn use_parking_hints(map: &mut raw_data::Map, shapes: ExtraShapes, gps_bounds: &GPSBounds) {
    // Match shapes with the nearest road + direction (true for forwards)
    let mut closest: FindClosest<(usize, bool)> = FindClosest::new(&gps_bounds.to_bounds());
    for (idx, r) in map.roads.iter().enumerate() {
        let pts = PolyLine::new(
            r.points
                .iter()
                .map(|pt| Pt2D::from_gps(*pt, gps_bounds).unwrap())
                .collect(),
        );

        closest.add((idx, true), &pts.shift_right(LANE_THICKNESS));
        closest.add((idx, false), &pts.shift_left(LANE_THICKNESS));
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
            if let Some(((r, fwds), _)) = closest.closest_pt(middle, 5.0 * LANE_THICKNESS * si::M) {
                let category = s.attributes.get("PARKING_CATEGORY");
                let has_parking = category != Some(&"None".to_string())
                    && category != Some(&"No Parking Allowed".to_string());
                // Blindly override prior values.
                if fwds {
                    map.roads[r].parking_lane_fwd = has_parking;
                } else {
                    map.roads[r].parking_lane_back = has_parking;
                }
            }
        }
    }
}
