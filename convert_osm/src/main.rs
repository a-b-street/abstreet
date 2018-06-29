// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate byteorder;
extern crate geom;
extern crate map_model;
extern crate ordered_float;
extern crate osm_xml;
#[macro_use]
extern crate structopt;

mod osm;
mod srtm;
mod traffic_signals;

use geom::LonLat;
use map_model::raw_data;
use ordered_float::NotNaN;
use srtm::Elevation;
use structopt::StructOpt;

const MAX_METERS_BTWN_INTERSECTION_AND_SIGNAL: f64 = 50.0;

#[derive(StructOpt, Debug)]
#[structopt(name = "convert_osm")]
struct Flags {
    /// OSM XML file to read
    #[structopt(long = "osm")]
    osm: String,

    /// HGT with elevation data
    #[structopt(long = "elevation")]
    elevation: String,

    /// SHP with traffic signals
    #[structopt(long = "traffic_signals")]
    traffic_signals: String,

    /// .abst with parcels, produced using the kml crate
    #[structopt(long = "parcels")]
    parcels: String,

    /// Output .abst path
    #[structopt(long = "output")]
    output: String,
}

fn main() {
    let flags = Flags::from_args();

    let elevation = Elevation::new(&flags.elevation).expect("loading .hgt failed");
    let (map, bounds) = osm::osm_to_raw_roads(&flags.osm);
    let mut map = osm::split_up_roads(&map, &elevation);
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

    for s in
        &traffic_signals::extract(&flags.traffic_signals).expect("loading traffic signals failed")
    {
        // Treat each point associated with the signal as a separate intersection. Later, we can
        // use this to treat multiple adjacent intersections as one logical intersection.
        for pt in &s.intersections {
            if bounds.contains(pt.x(), pt.y()) {
                let distance = |i: &raw_data::Intersection| {
                    // TODO weird to use Pt2D at all for GPS, uh oh
                    LonLat::new(pt.x(), pt.y())
                        .gps_dist_meters(LonLat::new(i.point.longitude, i.point.latitude))
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
    }

    println!("writing to {}", flags.output);
    abstutil::write_binary(&flags.output, &map).expect("serializing map failed");
}
