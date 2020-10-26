//! Extracts all cities from a large .osm file.
//!
//! 1) Reads a large .osm file
//! 2) Finds all boundary relations representing cities
//! 3) Calculates the polygon covering that city
//! 4) Uses osmconvert to clip the large .osm to a smaller one with just the city
//!
//! This tool writes all output files (.poly boundaries and .osm extracts) in the current
//! directory!

use abstutil::{CmdArgs, Timer};
use geom::{GPSBounds, LonLat, Polygon};
use std::process::Command;

fn main() {
    let mut args = CmdArgs::new();
    let input = args.required_free();
    args.done();
    let mut timer = Timer::new(format!("extract cities from {}", input));

    // Infer the boundary of the input from the <bounds> tag
    let doc = convert_osm::reader::read(&input, &GPSBounds::new(), &mut timer).unwrap();
    for (id, rel) in &doc.relations {
        if !rel.tags.is("border_type", "city") {
            continue;
        }
        let name = if let Some(name) = rel.tags.get("name") {
            name
        } else {
            println!("{} has no name?", id);
            continue;
        };

        println!("Found city relation for {}: {}", name, id);

        let polygons = convert_osm::osm_geom::glue_multipolygon(
            *id,
            convert_osm::osm_geom::get_multipolygon_members(*id, rel, &doc),
            None,
            &mut timer,
        );
        println!(
            "Extracted {} from {}, using the convex hull of {} polygons",
            name,
            id,
            polygons.len(),
        );
        let clip = Polygon::convex_hull(polygons);

        let clipping_polygon = format!("{}.poly", name);
        LonLat::write_osmosis_polygon(
            &clipping_polygon,
            &doc.gps_bounds.convert_back(clip.points()),
        )
        .unwrap();

        abstutil::must_run_cmd(
            Command::new("osmconvert")
                .arg(&input)
                .arg(format!("-B={}", clipping_polygon))
                .arg("--complete-ways")
                .arg(format!("-o={}.osm", name)),
        );
    }
}
