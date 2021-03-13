use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::Result;

use abstutil::{must_run_cmd, CmdArgs};

/// Import a one-shot A/B Street map in a single command. Takes a GeoJSON file with a boundary as
/// input. Automatically fetches the OSM data, clips it, and runs the importer.
/// TODO It currently overwrites a few fixed output files and doesn't clean them up.
fn main() -> Result<()> {
    let mut args = CmdArgs::new();
    let geojson_path = args.required_free();
    args.done();

    // TODO Assumes the binaries are in hardcoded target directories... we could be smarter and
    // detect if we're building from scratch, or look for stuff in the .zip release

    // Convert to a boundary polygon. This tool reads from STDIN.
    {
        let geojson = abstio::slurp_file(geojson_path)?;
        let mut cmd = Command::new("./target/debug/geojson_to_osmosis")
            .stdin(Stdio::piped())
            .spawn()?;
        let stdin = cmd.stdin.as_mut().unwrap();
        stdin.write_all(&geojson)?;
        assert!(cmd.wait()?.success());
    }

    // What file should we download?
    let url = {
        let out = Command::new("./target/debug/pick_geofabrik")
            .arg("boundary0.poly")
            .output()?;
        assert!(out.status.success());
        String::from_utf8(out.stdout)?
    };
    println!("go dl {}", url);

    // Download it!
    // TODO This is timing out. Also, really could use progress bars.
    {
        let resp = reqwest::blocking::get(&url)?;
        assert!(resp.status().is_success());
        let bytes = resp.bytes()?;
        let mut out = std::fs::File::create("raw.pbf")?;
        out.write_all(&bytes)?;
    }

    // Clip it
    must_run_cmd(
        Command::new("./target/release/clip_osm")
            .arg("--pbf=raw.pbf")
            .arg("--clip=boundary0.poly")
            .arg("--out=clipped.osm"),
    );

    // Import!
    must_run_cmd(
        Command::new("./target/release/importer")
            .arg("--oneshot=clipped.osm")
            .arg("--oneshot_clip=boundary0.poly"),
    );

    Ok(())
}
