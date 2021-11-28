use anyhow::Result;

use geom::LonLat;

pub fn run(path: String) -> Result<()> {
    let buffer = std::fs::read_to_string(path)?;
    for (idx, (points, maybe_name)) in LonLat::parse_geojson_polygons(buffer)?
        .into_iter()
        .enumerate()
    {
        let name = maybe_name.unwrap_or_else(|| format!("boundary{}", idx));
        let path = format!("{}.poly", name);
        LonLat::write_osmosis_polygon(&path, &points)?;
        println!("Wrote {}", path);
    }
    Ok(())
}
