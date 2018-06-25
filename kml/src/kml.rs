// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use map_model;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs::File;
use std::{io, f64};

pub fn load(
    path: &String,
    b: &map_model::Bounds,
) -> Result<Vec<map_model::raw_data::Parcel>, io::Error> {
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
                    let mut parcel = map_model::raw_data::Parcel { points: Vec::new() };
                    for pt in text.split(" ") {
                        if let Some((lon, lat)) = parse_pt(pt) {
                            if b.contains(lon, lat) {
                                parcel.points.push(map_model::raw_data::LatLon {
                                    latitude: lat,
                                    longitude: lon,
                                });
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
