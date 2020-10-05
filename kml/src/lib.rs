use abstutil::{prettyprint_usize, Timer};
use geom::{GPSBounds, LonLat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;

#[derive(Serialize, Deserialize)]
pub struct ExtraShapes {
    pub shapes: Vec<ExtraShape>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraShape {
    pub points: Vec<LonLat>,
    pub attributes: BTreeMap<String, String>,
}

pub fn load(
    path: &str,
    gps_bounds: &GPSBounds,
    require_all_pts_in_bounds: bool,
    timer: &mut Timer,
) -> Result<ExtraShapes, Box<dyn Error>> {
    timer.start(format!("read {}", path));
    let bytes = abstutil::slurp_file(path)?;
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
) -> Result<(), Box<dyn Error>> {
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
                    return Err(format!("Malformed coordinates: {}", pair).into());
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
    if coords.len() != 2 {
        return None;
    }
    match (coords[0].parse::<f64>(), coords[1].parse::<f64>()) {
        (Ok(lon), Ok(lat)) => Some(LonLat::new(lon, lat)),
        _ => None,
    }
}
