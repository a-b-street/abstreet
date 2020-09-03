use geojson::{GeoJson, Value};
use std::io::{self, Read};

/// Convert geojson boundary suitable for osmfilter and other osmosis based tools.
/// Expects the input to contain no element other than the boundary of interest.
//
/// Reads geojson text from stdin
/// Writes "poly" formatted text to stdout
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let geojson = buffer.parse::<GeoJson>()?;
    let points = boundary_coords(&geojson)?;

    println!("boundary");
    println!("1");
    for point in points {
        println!("     {}    {}", point[0], point[1]);
    }
    println!("END");
    println!("END");
    Ok(())
}

fn boundary_coords(geojson: &GeoJson) -> Result<Vec<Vec<f64>>, Box<dyn std::error::Error>> {
    let feature = match geojson {
        GeoJson::Feature(feature) => feature,
        GeoJson::FeatureCollection(feature_collection) => &feature_collection.features[0],
        _ => return Err(format!("Unexpected geojson: {:?}", geojson).into()),
    };

    match &feature.geometry.as_ref().map(|g| &g.value) {
        Some(Value::MultiPolygon(multi_polygon)) => return Ok(multi_polygon[0][0].clone()),
        Some(Value::Polygon(polygon)) => return Ok(polygon[0].clone()),
        _ => Err(format!("Unexpected feature: {:?}", feature).into()),
    }
}
