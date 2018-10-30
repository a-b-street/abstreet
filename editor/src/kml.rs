use abstutil::{FileWithProgress, Timer};
use geom::{GPSBounds, LonLat, PolyLine, Pt2D};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::BTreeMap;
use std::{f64, fmt, io};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ExtraShapeID(pub usize);

impl fmt::Display for ExtraShapeID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExtraShapeID({0})", self.0)
    }
}

#[derive(Debug)]
pub struct ExtraShape {
    pub id: ExtraShapeID,
    pub geom: ExtraShapeGeom,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum ExtraShapeGeom {
    Point(Pt2D),
    Points(PolyLine),
}

pub fn load(
    path: &str,
    gps_bounds: &GPSBounds,
    timer: &mut Timer,
) -> Result<Vec<ExtraShape>, io::Error> {
    info!("Opening {}", path);
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
                            let mut pts: Vec<Pt2D> = Vec::new();
                            for pair in text.split(" ") {
                                if let Some(pt) = parse_pt(pair, gps_bounds) {
                                    pts.push(pt);
                                } else {
                                    ok = false;
                                    break;
                                }
                            }
                            if ok && is_interesting_sign(&attributes) {
                                let id = ExtraShapeID(shapes.len());
                                shapes.push(ExtraShape {
                                    id,
                                    geom: if pts.len() == 1 {
                                        ExtraShapeGeom::Point(pts[0])
                                    } else {
                                        ExtraShapeGeom::Points(PolyLine::new(pts))
                                    },
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
        "Got {} shapes from {} and skipped {} shapes",
        shapes.len(),
        path,
        skipped_count
    );
    done(timer);
    return Ok(shapes);
}

fn parse_pt(input: &str, gps_bounds: &GPSBounds) -> Option<Pt2D> {
    let coords: Vec<&str> = input.split(",").collect();
    if coords.len() != 2 {
        return None;
    }
    return match (coords[0].parse::<f64>(), coords[1].parse::<f64>()) {
        (Ok(lon), Ok(lat)) => Pt2D::from_gps(LonLat::new(lon, lat), gps_bounds),
        _ => None,
    };
}

// TODO only for Street_Signs.kml; this is temporary to explore stuff
fn is_interesting_sign(attributes: &BTreeMap<String, String>) -> bool {
    true || attributes.get("CATEGORY") == Some(&"REGMIS".to_string())
}
