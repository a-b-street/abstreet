use anyhow::Result;

use geom::LonLat;

// TODO Temporarily making this to exactly the inverse!
pub fn run(path: String) -> Result<()> {
    let input = LonLat::read_osmosis_polygon(&path)?;
    let mut pts = Vec::new();
    for pt in input {
        pts.push(vec![pt.x(), pt.y()]);
    }
    let polygon = geojson::Geometry::new(geojson::Value::Polygon(vec![pts]));
    let gj = geom::geometries_with_properties_to_geojson(vec![(polygon, serde_json::Map::new())]);
    let new_path = path.replace(".poly", ".geojson");
    let _ = abstio::write_file(new_path, abstutil::to_json(&gj))?;
    Ok(())
}
