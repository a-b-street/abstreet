// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
    let bounds = map_model::Bounds {
        min_x: -122.4416,
        max_x: -122.2421,
        min_y: 47.5793,
        max_y: 47.7155,
    };
    let mut map = map_model::pb::Map::new();
    for p in kml::load(&args[1], &bounds).unwrap().iter() {
        // TODO dont clone, take ownership!
        map.mut_parcels().push(p.clone());
    }

    let out_path = &args[2];
    println!("writing to {}", out_path);
    map_model::write_pb(&map, out_path).expect("serializing map failed");
}
