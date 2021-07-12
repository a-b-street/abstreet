use std::io::{self, Read};

use anyhow::Result;

use geom::LonLat;

/// Reads GeoJSON input from STDIN, extracts a polygon from every feature, and writes numbered
/// files in the https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format format as
/// output.
fn main() -> Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
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
