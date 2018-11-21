extern crate abstutil;
extern crate geom;
#[macro_use]
extern crate log;
extern crate quick_xml;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

use abstutil::{FileWithProgress, Timer};
use geom::{GPSBounds, LonLat};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::BTreeMap;
use std::io;

#[derive(StructOpt)]
#[structopt(name = "kml")]
pub struct Flags {
    /// KML file to read
    #[structopt(long = "input")]
    pub input: String,

    /// Output (serialized ExtraShapes) to write
    #[structopt(long = "output")]
    pub output: String,
}

#[derive(Serialize, Deserialize)]
pub struct ExtraShapes {
    pub shapes: Vec<ExtraShape>,
}

#[derive(Serialize, Deserialize)]
pub struct ExtraShape {
    pub points: Vec<LonLat>,
    pub attributes: BTreeMap<String, String>,
}

pub fn load(
    path: &str,
    gps_bounds: &GPSBounds,
    timer: &mut Timer,
) -> Result<ExtraShapes, io::Error> {
    info!(target: "kml", "Opening {}", path);
    let (f, done) = FileWithProgress::new(path)?;
    // TODO FileWithProgress should implement BufRead, so we don't have to double wrap like this
    let mut reader = Reader::from_reader(io::BufReader::new(f));
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
                            for pair in text.split(" ") {
                                if let Some(pt) = parse_pt(pair, &gps_bounds) {
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

    info!(
        target: "kml",
        "Got {} shapes from {} and skipped {} shapes",
        shapes.len(),
        path,
        skipped_count
    );
    done(timer);
    return Ok(ExtraShapes { shapes });
}

fn parse_pt(input: &str, gps_bounds: &GPSBounds) -> Option<LonLat> {
    let coords: Vec<&str> = input.split(",").collect();
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

/*
// TODO only for Street_Signs.kml; this is temporary to explore stuff
fn is_interesting_sign(attributes: &BTreeMap<String, String>) -> bool {
    attributes.get("CATEGORY") == Some(&"REGMIS".to_string())
}
*/
