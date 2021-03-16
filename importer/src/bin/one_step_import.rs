use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Result;

use abstio::CityName;
use abstutil::{must_run_cmd, CmdArgs};

/// Import a one-shot A/B Street map in a single command. Takes a GeoJSON file with a boundary as
/// input. Automatically fetches the OSM data, clips it, and runs the importer.
#[tokio::main]
async fn main() -> Result<()> {
    let mut args = CmdArgs::new();
    let geojson_path = args.required_free();
    let drive_on_left = args.enabled("--drive_on_left");
    args.done();

    // Handle running from a binary release or from git. If the latter and the user hasn't built
    // the tools, they'll get an error.
    let bin_dir = vec![
        "./target/release",
        "../target/release",
        "../../target/release",
        "./tools",
        "../tools",
    ]
    .into_iter()
    .find(|x| Path::new(x).exists())
    .expect("Can't find target/ or tools/ directory");
    println!("Found other executables at {}", bin_dir);

    // Convert to a boundary polygon. This tool reads from STDIN.
    {
        println!("Converting GeoJSON to Osmosis boundary");
        let geojson = abstio::slurp_file(geojson_path)?;
        let mut cmd = Command::new(format!("{}/geojson_to_osmosis", bin_dir))
            .stdin(Stdio::piped())
            .spawn()?;
        let stdin = cmd.stdin.as_mut().unwrap();
        stdin.write_all(&geojson)?;
        assert!(cmd.wait()?.success());
    }

    // What file should we download?
    let url = {
        println!("Figuring out what Geofabrik file contains your boundary");
        let out = Command::new(format!("{}/pick_geofabrik", bin_dir))
            .arg("boundary0.poly")
            .output()?;
        assert!(out.status.success());
        String::from_utf8(out.stdout)?
    };

    // Name the temporary map based on the Geofabrik region.
    let city = CityName::new("zz", "oneshot");
    let name = abstutil::basename(&url)
        .strip_suffix("-latest.osm")
        .unwrap()
        .to_string();
    let pbf = city.input_path(format!("osm/{}.pbf", abstutil::basename(&url)));
    let osm = city.input_path(format!("osm/{}.osm", name));
    std::fs::create_dir_all(std::path::Path::new(&osm).parent().unwrap())
        .expect("Creating parent dir failed");

    // Download it!
    // TODO This is timing out. Also, really could use progress bars.
    if !abstio::file_exists(&pbf) {
        println!("Downloading {}", url);
        abstio::download_to_file(url, pbf.clone(), false).await?;
    }

    // Clip it
    println!("Clipping osm.pbf file to your boundary");
    must_run_cmd(
        Command::new(format!("{}/clip_osm", bin_dir))
            .arg(format!("--pbf={}", pbf))
            .arg("--clip=boundary0.poly")
            .arg(format!("--out={}", osm)),
    );

    // Import!
    {
        let mut cmd = Command::new(format!("{}/importer", bin_dir));
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

    // For the sake of the UI, print the name of the new map as the last line of output.
    println!("{}", name);

    Ok(())
}
