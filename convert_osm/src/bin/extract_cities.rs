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
use geom::{GPSBounds, LonLat};
use std::fs::File;
use std::io::{Error, Write};
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
            "Extracted {} polygons for {} from {}, keeping the largest",
            polygons.len(),
            name,
            id
        );
        let largest = polygons
            .into_iter()
            .max_by_key(|p| p.area() as usize)
            .unwrap();

        let clipping_polygon = format!("{}.poly", name);
        write_osmosis_polygon(
            &clipping_polygon,
            doc.gps_bounds.convert_back(largest.points()),
        )
        .unwrap();

        run(Command::new("osmconvert")
            .arg(&input)
            .arg(format!("-B={}", clipping_polygon))
            .arg("--complete-ways")
            .arg(format!("-o={}.osm", name)));
    }
}

// TODO Refactor with the devtools/polygon variant and the geojson_to_osmosis tool
fn write_osmosis_polygon(path: &str, pts: Vec<LonLat>) -> Result<(), Error> {
    let mut f = File::create(path)?;
    writeln!(f, "boundary")?;
    writeln!(f, "1")?;
    for pt in pts {
        writeln!(f, "     {}    {}", pt.x(), pt.y())?;
    }
    writeln!(f, "END")?;
    writeln!(f, "END")?;
    Ok(())
}

// TODO Refactor to abstutil
// Runs a command, asserts success. STDOUT and STDERR aren't touched.
fn run(cmd: &mut Command) {
    println!("- Running {:?}", cmd);
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                panic!("{:?} failed", cmd);
            }
        }
        Err(err) => {
            panic!("Failed to run {:?}: {:?}", cmd, err);
        }
    }
}
