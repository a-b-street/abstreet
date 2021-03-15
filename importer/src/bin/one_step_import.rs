use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Result;

use abstio::CityName;
use abstutil::{must_run_cmd, CmdArgs};

/// Import a one-shot A/B Street map in a single command. Takes a GeoJSON file with a boundary as
/// input. Automatically fetches the OSM data, clips it, and runs the importer.
/// TODO It currently overwrites a few fixed output files and doesn't clean them up.
fn main() -> Result<()> {
    let mut args = CmdArgs::new();
    let geojson_path = args.required_free();
    let drive_on_left = args.enabled("--drive_on_left");
    args.done();

    // TODO Assume we're running from git and all the tools are built in the appropriate directory.
    let bin_dir = if Path::new("target").exists() {
        "./target"
    } else if Path::new("../target").exists() {
        "../target"
    } else if Path::new("../../target").exists() {
        "../../target"
    } else {
        panic!("Can't find target/ directory");
    };

    // Convert to a boundary polygon. This tool reads from STDIN.
    {
        println!("Converting GeoJSON to Osmosis boundary");
        let geojson = abstio::slurp_file(geojson_path)?;
        let mut cmd = Command::new(format!("{}/debug/geojson_to_osmosis", bin_dir))
            .stdin(Stdio::piped())
            .spawn()?;
        let stdin = cmd.stdin.as_mut().unwrap();
        stdin.write_all(&geojson)?;
        assert!(cmd.wait()?.success());
    }

    // What file should we download?
    let url = {
        println!("Figuring out what Geofabrik file contains your boundary");
        let out = Command::new(format!("{}/debug/pick_geofabrik", bin_dir))
            .arg("boundary0.poly")
            .output()?;
        assert!(out.status.success());
        String::from_utf8(out.stdout)?
    };

    // Name the temporary map based on the Geofabrik region.
    let name = CityName::new("zz", "oneshot");
    let pbf = name.input_path(format!("osm/{}.pbf", abstutil::basename(&url)));
    let osm = name.input_path(format!(
        "osm/{}.osm",
        abstutil::basename(&url)
            .strip_suffix("-latest.osm")
            .unwrap()
    ));
    std::fs::create_dir_all(std::path::Path::new(&osm).parent().unwrap())
        .expect("Creating parent dir failed");

    // Download it!
    // TODO This is timing out. Also, really could use progress bars.
    if !abstio::file_exists(&pbf) {
        println!("Downloading {}", url);
        let resp = reqwest::blocking::get(&url)?;
        assert!(resp.status().is_success());
        let bytes = resp.bytes()?;
        let mut out = std::fs::File::create(&pbf)?;
        out.write_all(&bytes)?;
    }

    // Clip it
    println!("Clipping osm.pbf file to your boundary");
    must_run_cmd(
        Command::new(format!("{}/release/clip_osm", bin_dir))
            .arg(format!("--pbf={}", pbf))
            .arg("--clip=boundary0.poly")
            .arg(format!("--out={}", osm)),
    );

    // Import!
    {
        let mut cmd = Command::new(format!("{}/release/importer", bin_dir));
        cmd.arg(format!("--oneshot={}", osm));
        cmd.arg("--oneshot_clip=boundary0.poly");
        if drive_on_left {
            cmd.arg("--oneshot_drive_on_left");
        }
        println!("Running importer");
        must_run_cmd(&mut cmd);
    }

    // Clean up temporary files. If we broke before this, deliberately leave them around for
    // debugging.
    abstio::delete_file("boundary0.poly");

    Ok(())
}
