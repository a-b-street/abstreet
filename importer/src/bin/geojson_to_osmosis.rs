use std::io::{self, Read};

use anyhow::Result;
use geojson::{GeoJson, Value};

use geom::LonLat;

/// Reads GeoJSON input from STDIN, extracts a polygon from every feature, and writes numbered
/// files in the https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format format as
/// output.
fn main() -> Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let geojson = buffer.parse::<GeoJson>()?;

    for (idx, points) in extract_boundaries(geojson)?.into_iter().enumerate() {
        let path = format!("boundary{}.poly", idx);
        LonLat::write_osmosis_polygon(&path, &points)?;
        println!("Wrote {}", path);
    }
    Ok(())
}

fn extract_boundaries(geojson: GeoJson) -> Result<Vec<Vec<LonLat>>> {
    let features = match geojson {
        GeoJson::Feature(feature) => vec![feature],
        GeoJson::FeatureCollection(feature_collection) => feature_collection.features,
        _ => anyhow::bail!("Unexpected geojson: {:?}", geojson),
    };
    let mut polygons = Vec::new();
    for mut feature in features {
        let points = match feature.geometry.take().map(|g| g.value) {
            Some(Value::MultiPolygon(multi_polygon)) => multi_polygon[0][0].clone(),
            Some(Value::Polygon(polygon)) => polygon[0].clone(),
            _ => {
                anyhow::bail!("Unexpected feature: {:?}", feature);
            }
        };
        polygons.push(
            points
                .into_iter()
                .map(|pt| LonLat::new(pt[0], pt[1]))
                .collect(),
        );
    }
    Ok(polygons)
}
