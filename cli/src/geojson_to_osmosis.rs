use anyhow::Result;

use geom::LonLat;

pub fn run(path: String) -> Result<()> {
    let buffer = fs_err::read_to_string(path)?;
    for (idx, (points, maybe_name)) in LonLat::parse_geojson_polygons(buffer)?
        .into_iter()
        .enumerate()
    {
        let name = maybe_name.unwrap_or_else(|| format!("boundary{}", idx));
        // Canonicalize the filename
        let name = name.to_ascii_lowercase().replace(" ", "_");
        let path = format!("{}.poly", name);
        LonLat::write_osmosis_polygon(&path, &points)?;
        println!("Wrote {}", path);
    }
    Ok(())
}
