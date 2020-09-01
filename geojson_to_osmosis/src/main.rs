use std::io::{self, Read};
use geojson::{GeoJson, Value};

/// Convert geojson boundary suitable for osmfilter and other osmosis based tools.
/// Expects the input to no elements other than the boundary of interest.
//
/// Reads geojson text from stdin
/// Writes "poly" formatted text to stdout
fn main() -> Result<(), Error> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let geojson = buffer.parse::<GeoJson>()?;
    let points = boundary_coords(&geojson)?;

    println!("map_name");
    println!("1");
    for point in points {
        println!("     {}    {}", point[0], point[1]);
    }
    println!("END");
    println!("END");
    Ok(())
}

fn boundary_coords(geojson: &GeoJson) -> Result<Vec<Vec<f64>>, Error> {
    let feature = match geojson {
        GeoJson::Feature(feature) => feature,
        GeoJson::FeatureCollection(feature_collection) => &feature_collection.features[0],
        _ => return Err(Error::BadInput(format!("Unexpected geojson: {:?}", geojson)))
    };

    match &feature.geometry.as_ref().map(|g| &g.value) {
        Some(Value::MultiPolygon(multi_polygon)) => {
            return Ok(multi_polygon[0][0].clone())
        },
        Some(Value::Polygon(polygon)) => {
            return Ok(polygon[0].clone())
        },
        _ => Err(Error::BadInput(format!("Unexpected feature: {:?}", feature)))
    }
}

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    GeoJson(geojson::Error),
    BadInput(String),
}

impl std::convert::From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

impl std::convert::From<geojson::Error> for Error {
    fn from(e: geojson::Error) -> Error {
        Error::GeoJson(e)
    }
}

