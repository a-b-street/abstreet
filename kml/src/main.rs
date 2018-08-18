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
    let bounds = geom::Bounds {
        min_x: -122.4416,
        max_x: -122.2421,
        min_y: 47.5793,
        max_y: 47.7155,
        represents_world_space: false,
    };
    // TODO could use a better output format now
    let mut map = map_model::raw_data::Map::blank();
    if let Ok(parcels) = kml::load(&args[1], &bounds) {
        map.parcels.extend(parcels);
    }

    let out_path = &args[2];
    println!("writing to {}", out_path);
    abstutil::write_binary(out_path, &map).expect("serializing map failed");
}
