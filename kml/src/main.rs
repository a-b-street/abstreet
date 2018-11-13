// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate geom;
extern crate map_model;
extern crate quick_xml;

mod kml;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("Gimme a .kml and a .abst");
        process::exit(1);
    }

    // TODO don't hardcode
    let mut bounds = geom::GPSBounds::new();
    bounds.update(geom::LonLat::new(-122.4416, 47.5793));
    bounds.update(geom::LonLat::new(-122.2421, 47.7155));
    // TODO could use a better output format now
    let mut map = map_model::raw_data::Map::blank();
    if let Ok(parcels) = kml::load(&args[1], &bounds) {
        map.parcels.extend(parcels);
    }

    let out_path = &args[2];
    println!("writing to {}", out_path);
    abstutil::write_binary(out_path, &map).expect("serializing map failed");
}
