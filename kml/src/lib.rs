// TODO Time to rename this crate

#[macro_use]
extern crate anyhow;

use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::{prettyprint_usize, Timer};
use geom::{GPSBounds, LonLat};

/// Some dataset imported from KML, CSV, or something else. If the dataset is large, converting to
/// this format and serializing is faster than parsing the original again.
#[derive(Serialize, Deserialize)]
pub struct ExtraShapes {
    pub shapes: Vec<ExtraShape>,
}

/// A single object in the dataset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraShape {
    /// The object has a different inferred shape depending on the points:
    /// - a single point just represents a position
    /// - a ring of points (with the first and last matching) is interpreted as a polygon
    /// - multiple points are interpreted as a PolyLine
    pub points: Vec<LonLat>,
    /// Arbitrary key/value pairs associated with this object; no known schema.
    pub attributes: BTreeMap<String, String>,
}

/// Parses a .kml file and returns ExtraShapes. Objects will be clipped to the given gps_bounds. If
/// require_all_pts_in_bounds is true, objects that're partly out-of-bounds will be skipped.
pub fn load(
    path: &str,
    gps_bounds: &GPSBounds,
    require_all_pts_in_bounds: bool,
    timer: &mut Timer,
) -> Result<ExtraShapes> {
    timer.start(format!("read {}", path));
    let bytes = abstio::slurp_file(path)?;
    let raw_string = std::str::from_utf8(&bytes)?;
    let tree = roxmltree::Document::parse(raw_string)?;
    timer.stop(format!("read {}", path));

    let mut shapes = Vec::new();
    let mut skipped_count = 0;
    let mut kv = BTreeMap::new();

    timer.start("scrape objects");
    recurse(
        tree.root(),
        &mut shapes,
        &mut skipped_count,
        &mut kv,
        gps_bounds,
        require_all_pts_in_bounds,
    )?;
    timer.stop("scrape objects");

    timer.note(format!(
        "Got {} shapes from {} and skipped {} shapes",
        prettyprint_usize(shapes.len()),
        path,
        prettyprint_usize(skipped_count)
    ));

    Ok(ExtraShapes { shapes })
}

fn recurse(
    node: roxmltree::Node,
    shapes: &mut Vec<ExtraShape>,
    skipped_count: &mut usize,
    kv: &mut BTreeMap<String, String>,
    gps_bounds: &GPSBounds,
    require_all_pts_in_bounds: bool,
) -> Result<()> {
    for child in node.children() {
        recurse(
            child,
            shapes,
            skipped_count,
            kv,
            gps_bounds,
            require_all_pts_in_bounds,
        )?;
    }
    if node.tag_name().name() == "SimpleData" {
        let key = node.attribute("name").unwrap().to_string();
        let value = node
            .text()
            .map(|x| x.to_string())
            .unwrap_or_else(String::new);
        kv.insert(key, value);
    } else if node.tag_name().name() == "coordinates" {
        let mut any_oob = false;
        let mut any_ok = false;
        let mut pts: Vec<LonLat> = Vec::new();
        if let Some(txt) = node.text() {
            for pair in txt.split(' ') {
                if let Some(pt) = parse_pt(pair) {
                    pts.push(pt);
                    if gps_bounds.contains(pt) {
                        any_ok = true;
                    } else {
                        any_oob = true;
                    }
                } else {
                    bail!("Malformed coordinates: {}", pair);
                }
            }
        }
        if any_ok && (!any_oob || !require_all_pts_in_bounds) {
            let attributes = std::mem::replace(kv, BTreeMap::new());
            shapes.push(ExtraShape {
                points: pts,
                attributes,
            });
        } else {
            *skipped_count += 1;
        }
    }
    Ok(())
}

fn parse_pt(input: &str) -> Option<LonLat> {
    let coords: Vec<&str> = input.split(',').collect();
    // Normally each coordinate is just (X, Y), but for census tract files, there's a third Z
    // component that's always 0. Just ignore it.
    if coords.len() < 2 {
        return None;
    }
    match (coords[0].parse::<f64>(), coords[1].parse::<f64>()) {
        (Ok(lon), Ok(lat)) => Some(LonLat::new(lon, lat)),
        _ => None,
    }
}

impl ExtraShapes {
    /// Parses a .csv file and returns ExtraShapes. Each record must have a column called
    /// 'Longitude' and 'Latitude', representing a single point; all other columns will just be
    /// attributes. Objects will be clipped to the given gps_bounds.
    pub fn load_csv(path: &str, gps_bounds: &GPSBounds, timer: &mut Timer) -> Result<ExtraShapes> {
        timer.start(format!("read {}", path));
        let mut shapes = Vec::new();
        for rec in csv::Reader::from_path(path)?.deserialize() {
            let mut rec: BTreeMap<String, String> = rec?;
            match (rec.remove("Longitude"), rec.remove("Latitude")) {
                (Some(lon), Some(lat)) => {
                    if let (Ok(lon), Ok(lat)) = (lon.parse::<f64>(), lat.parse::<f64>()) {
                        let pt = LonLat::new(lon, lat);
                        if gps_bounds.contains(pt) {
                            shapes.push(ExtraShape {
                                points: vec![pt],
                                attributes: rec,
                            });
                        }
                    }
                }
                _ => {
                    timer.stop(format!("read {}", path));
                    bail!(
                        "{} doesn't have a column called Longitude and Latitude",
                        path
                    )
                }
            }
        }
        timer.stop(format!("read {}", path));
        Ok(ExtraShapes { shapes })
    }
}
