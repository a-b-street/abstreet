extern crate aabb_quadtree;
extern crate abstutil;
extern crate byteorder;
extern crate dimensioned;
extern crate geo;
extern crate geojson;
extern crate geom;
extern crate gtfs;
extern crate map_model;
extern crate ordered_float;
extern crate osm_xml;
extern crate shp;
// TODO To serialize Neighborhoods. Should probably lift this into the map_model layer instead of
// have this weird dependency.
extern crate sim;
#[macro_use]
extern crate structopt;

mod group_parcels;
mod neighborhoods;
mod osm;
mod remove_disconnected;
mod split_ways;
mod srtm;
mod traffic_signals;

use map_model::raw_data;
use ordered_float::NotNaN;
use srtm::Elevation;
use std::path::Path;

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

    /// SHP with traffic signals
    #[structopt(long = "traffic_signals")]
    pub traffic_signals: String,

    /// .abst with parcels, produced using the kml crate
    #[structopt(long = "parcels")]
    pub parcels: String,

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

    println!("Loading parcels from {}", flags.parcels);
    let parcels_map: raw_data::Map =
        abstutil::read_binary(&flags.parcels, timer).expect("loading parcels failed");
    println!(
        "Finding matching parcels from {} candidates",
        parcels_map.parcels.len()
    );
    for p in parcels_map.parcels {
        if p.points
            .iter()
            .find(|pt| !gps_bounds.contains(**pt))
            .is_none()
        {
            map.parcels.push(p);
        }
    }
    group_parcels::group_parcels(&gps_bounds, &mut map.parcels);

    for pt in traffic_signals::extract(&flags.traffic_signals)
        .expect("loading traffic signals failed")
        .into_iter()
    {
        if gps_bounds.contains(pt) {
            let distance = |i: &raw_data::Intersection| pt.gps_dist_meters(i.point);

            // TODO use a quadtree or some better way to match signals to the closest
            // intersection
            let closest_intersection = map
                .intersections
                .iter_mut()
                .min_by_key(|i| NotNaN::new(distance(i)).unwrap())
                .unwrap();
            let dist = distance(closest_intersection);
            if dist <= MAX_METERS_BTWN_INTERSECTION_AND_SIGNAL {
                if closest_intersection.has_traffic_signal {
                    println!("WARNING: {:?} already has a traffic signal, but there's another one that's {} from it", closest_intersection, dist);
                }
                closest_intersection.has_traffic_signal = true;
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
