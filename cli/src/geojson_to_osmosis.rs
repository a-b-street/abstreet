use anyhow::Result;

use geom::LonLat;

pub fn run(path: String) -> Result<()> {
    let buffer = std::fs::read_to_string(path)?;
    for (idx, points) in LonLat::parse_geojson_polygons(buffer)?
        .into_iter()
        .enumerate()
    {
        let path = format!("boundary{}.poly", idx);
        LonLat::write_osmosis_polygon(&path, &points)?;
        println!("Wrote {}", path);
    }
    Ok(())
}
