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

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs::File;
use std::{io, f64};

pub fn load(path: &String, b: &map_model::Bounds) -> Result<Vec<map_model::pb::Parcel>, io::Error> {
    println!("Opening {}", path);
    let f = File::open(path).unwrap();
    let mut reader = Reader::from_reader(io::BufReader::new(f));
    reader.trim_text(true);

    let mut parcels = Vec::new();
    let mut buf = Vec::new();
    let mut last_progress_byte = 0;
    loop {
        if reader.buffer_position() - last_progress_byte >= 1024 * 1024 * 10 {
            last_progress_byte = reader.buffer_position();
            println!(
                "Processed {} MB of {}",
                last_progress_byte / (1024 * 1024),
                path
            );
        }
        match reader.read_event(&mut buf) {
            Ok(Event::Text(e)) => {
                let text = e.unescape_and_decode(&reader).unwrap();
                // We can be incredibly lazy here and just interpret all text as coordinates. The
                // other metadata for each placemark doesn't look useful yet. We do have to
                // interpret parsing failures appropriately though...
                if text.contains(" ") {
                    let mut ok = true;
                    let mut parcel = map_model::pb::Parcel::new();
                    for pt in text.split(" ") {
                        if let Some((lon, lat)) = parse_pt(pt) {
                            if b.contains(lon, lat) {
                                let mut coord = map_model::pb::Coordinate::new();
                                coord.set_longitude(lon);
                                coord.set_latitude(lat);
                                parcel.mut_points().push(coord);
                            } else {
                                ok = false;
                            }
                        } else {
                            ok = false;
                        }
                    }
                    if ok {
                        parcels.push(parcel);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!(
                "XML error at position {}: {:?}",
                reader.buffer_position(),
                e
            ),
            _ => (),
        }
        buf.clear();
    }
    return Ok(parcels);
}

fn parse_pt(input: &str) -> Option<(f64, f64)> {
    let coords: Vec<&str> = input.split(",").collect();
    if coords.len() != 2 {
        return None;
    }
    return match (coords[0].parse::<f64>(), coords[1].parse::<f64>()) {
        (Ok(lon), Ok(lat)) => Some((lon, lat)),
        _ => None,
    };
}
