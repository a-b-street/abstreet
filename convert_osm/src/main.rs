// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate byteorder;
extern crate map_model;
extern crate ordered_float;
extern crate osm_xml;
#[macro_use]
extern crate structopt;

mod osm;
mod srtm;
mod traffic_signals;

use map_model::{pb, Pt2D};
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
    let parcels_map = map_model::load_pb(&flags.parcels).expect("loading parcels failed");
    println!(
        "Finding matching parcels from {} candidates",
        parcels_map.get_parcels().len()
    );
    for p in parcels_map.get_parcels() {
        if p.get_points()
            .iter()
            .find(|pt| !bounds.contains(pt.get_longitude(), pt.get_latitude()))
            .is_none()
        {
            map.mut_parcels().push(p.clone());
        }
    }

    for s in
        &traffic_signals::extract(&flags.traffic_signals).expect("loading traffic signals failed")
    {
        // Treat each point associated with the signal as a separate intersection. Later, we can
        // use this to treat multiple adjacent intersections as one logical intersection.
        for pt in &s.intersections {
            if bounds.contains(pt.x(), pt.y()) {
                let distance = |i: &pb::Intersection| {
                    pt.gps_dist_meters(&Pt2D::new(
                        i.get_point().get_longitude(),
                        i.get_point().get_latitude(),
                    ))
                };

                // TODO use a quadtree or some better way to match signals to the closest
                // intersection
                let closest_intersection = map.mut_intersections()
                    .iter_mut()
                    .min_by_key(|i| NotNaN::new(distance(i)).unwrap())
                    .unwrap();
                let dist = distance(closest_intersection);
                if dist <= MAX_METERS_BTWN_INTERSECTION_AND_SIGNAL {
                    if closest_intersection.get_has_traffic_signal() {
                        println!("WARNING: {:?} already has a traffic signal, but there's another one that's {} from it", closest_intersection, dist);
                    }
                    closest_intersection.set_has_traffic_signal(true);
                }
            }
        }
    }

    println!("writing to {}", flags.output);
    map_model::write_pb(&map, &flags.output).expect("serializing map failed");
}
