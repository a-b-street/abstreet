extern crate abstutil;
extern crate byteorder;
extern crate geom;
extern crate map_model;
extern crate ordered_float;
extern crate osm_xml;
#[macro_use]
extern crate pretty_assertions;
extern crate shp;
#[macro_use]
extern crate structopt;

mod osm;
mod remove_disconnected;
mod split_ways;
mod srtm;
mod traffic_signals;

use geom::LonLat;
use map_model::raw_data;
use ordered_float::NotNaN;
use srtm::Elevation;

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

    /// Output .abst path
    #[structopt(long = "output")]
    pub output: String,
}

pub fn convert(flags: &Flags) -> raw_data::Map {
    let elevation = Elevation::new(&flags.elevation).expect("loading .hgt failed");
    let raw_map = osm::osm_to_raw_roads(&flags.osm);
    let mut map = split_ways::split_up_roads(&raw_map, &elevation);
    remove_disconnected::remove_disconnected_roads(&mut map);
    let bounds = map.get_gps_bounds();

    println!("Loading parcels from {}", flags.parcels);
    let parcels_map: raw_data::Map =
        abstutil::read_binary(&flags.parcels).expect("loading parcels failed");
    println!(
        "Finding matching parcels from {} candidates",
        parcels_map.parcels.len()
    );
    for p in parcels_map.parcels {
        if p.points
            .iter()
            .find(|pt| !bounds.contains(pt.longitude, pt.latitude))
            .is_none()
        {
            map.parcels.push(p);
        }
    }

    for coord in
        &traffic_signals::extract(&flags.traffic_signals).expect("loading traffic signals failed")
    {
        if bounds.contains(coord.longitude, coord.latitude) {
            let distance = |i: &raw_data::Intersection| {
                coord.gps_dist_meters(LonLat::new(i.point.longitude, i.point.latitude))
            };

            // TODO use a quadtree or some better way to match signals to the closest
            // intersection
            let closest_intersection = map.intersections
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

    map
}
