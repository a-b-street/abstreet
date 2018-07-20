// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use geom::LonLat;
use shp;
use std;
use std::io::Error;

// Returns the location of traffic signals
pub fn extract(path: &str) -> Result<Vec<LonLat>, Error> {
    println!("Opening {}", path);

    let reader = shp::ShpReader::open(path)?;
    let file = reader.read();
    let mut result: Vec<LonLat> = Vec::new();
    for r in &file.records {
        if let shp::ShpRecordContent::MultiPoint(ref raw_shape) = r.content {
            // The shp crate doesn't expose the struct fields as public. Send a PR later, do this
            // workaround for now.
            let shape = unsafe {
                std::mem::transmute::<&shp::MultiPointShape, &MultiPointShape>(raw_shape)
            };
            // Some intersections have multiple points, which shows complicated intersections. For
            // now, don't handle these.
            result.push(LonLat::new(shape.points[0].x, shape.points[0].y));
        } else {
            println!("Unexpected shp record: {:?}", r.content);
        }
    }

    Ok(result)
}

struct PointShape {
    x: f64,
    y: f64,
}

struct MultiPointShape {
    _xmin: f64,
    _xmax: f64,
    _ymin: f64,
    _ymax: f64,
    _num_points: i32,
    points: Vec<PointShape>,
}
