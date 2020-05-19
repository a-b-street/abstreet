use abstutil::{prettyprint_usize, FileWithProgress, Timer};
use geom::{GPSBounds, LonLat};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
pub struct ExtraShapes {
    pub shapes: Vec<ExtraShape>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtraShape {
    pub points: Vec<LonLat>,
    pub attributes: BTreeMap<String, String>,
}

pub fn load(
    path: &str,
    gps_bounds: &GPSBounds,
    timer: &mut Timer,
) -> Result<ExtraShapes, std::io::Error> {
    println!("Opening {}", path);
    let (f, done) = FileWithProgress::new(path)?;
    // TODO FileWithProgress should implement BufRead, so we don't have to double wrap like this
    let mut reader = Reader::from_reader(std::io::BufReader::new(f));
    reader.trim_text(true);

    let mut buf = Vec::new();

    // TODO uncomfortably stateful
    let mut shapes = Vec::new();
    let mut scanned_schema = false;
    let mut attributes: BTreeMap<String, String> = BTreeMap::new();
    let mut attrib_key: Option<String> = None;

    let mut skipped_count = 0;

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.unescape_and_decode(&reader).unwrap();
                if name == "Placemark" {
                    scanned_schema = true;
                } else if name.starts_with("SimpleData name=\"") {
                    attrib_key = Some(name["SimpleData name=\"".len()..name.len() - 1].to_string());
                } else if name == "coordinates" {
                    attrib_key = Some(name);
                } else {
                    attrib_key = None;
                }
            }
            Ok(Event::Text(e)) => {
                if scanned_schema {
                    if let Some(ref key) = attrib_key {
                        let text = e.unescape_and_decode(&reader).unwrap();
                        if key == "coordinates" {
                            let mut ok = true;
                            let mut pts: Vec<LonLat> = Vec::new();
                            for pair in text.split(' ') {
                                if let Some(pt) = parse_pt(pair, gps_bounds) {
                                    pts.push(pt);
                                } else {
                                    ok = false;
                                    break;
                                }
                            }
                            if ok {
                                shapes.push(ExtraShape {
                                    points: pts,
                                    attributes: attributes.clone(),
                                });
                            } else {
                                skipped_count += 1;
                            }
                            attributes.clear();
                        } else {
                            attributes.insert(key.to_string(), text);
                        }
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

    println!(
        "Got {} shapes from {} and skipped {} shapes",
        prettyprint_usize(shapes.len()),
        path,
        prettyprint_usize(skipped_count)
    );
    done(timer);

    Ok(ExtraShapes { shapes })
}

fn parse_pt(input: &str, gps_bounds: &GPSBounds) -> Option<LonLat> {
    let coords: Vec<&str> = input.split(',').collect();
    if coords.len() != 2 {
        return None;
    }
    let pt = match (coords[0].parse::<f64>(), coords[1].parse::<f64>()) {
        (Ok(lon), Ok(lat)) => Some(LonLat::new(lon, lat)),
        _ => None,
    }?;
    if gps_bounds.contains(pt) {
        Some(pt)
    } else {
        None
    }
}
