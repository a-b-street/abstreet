use std::path::Path;
use std::process::Command;

use abstio::MapName;
use abstutil::{must_run_cmd, Timer};

use crate::configuration::ImporterConfiguration;

// If the output file doesn't already exist, downloads the URL into that location. Automatically
// uncompresses .zip and .gz files.
pub fn download(config: &ImporterConfiguration, output: &str, url: &str) {
    let output = abstio::path(output);
    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the directory
    std::fs::create_dir_all(Path::new(&output).parent().unwrap())
        .expect("Creating parent dir failed");

    let tmp = "tmp_output";
    println!("- Missing {}, so downloading {}", output, url);
    must_run_cmd(
        Command::new("curl")
            .arg("--fail")
            .arg("-L")
            .arg("-o")
            .arg(tmp)
            .arg(url),
    );

    // Argh the Dropbox URL is .zip?dl=0
    if url.contains(".zip") {
        let unzip_to = if output.ends_with("/") {
            output
        } else {
            Path::new(&output).parent().unwrap().display().to_string()
        };
        println!("- Unzipping into {}", unzip_to);
        must_run_cmd(Command::new(&config.unzip).arg(tmp).arg("-d").arg(unzip_to));
        std::fs::remove_file(tmp).unwrap();
    } else if url.ends_with(".gz") {
        println!("- Gunzipping");
        std::fs::rename(tmp, format!("{}.gz", output)).unwrap();

        let mut gunzip_cmd = Command::new(&config.gunzip);
        for arg in config.gunzip_args.split_ascii_whitespace() {
            gunzip_cmd.arg(arg);
        }
        must_run_cmd(gunzip_cmd.arg(format!("{}.gz", output)));
    } else {
        std::fs::rename(tmp, output).unwrap();
    }
}

// If the output file doesn't already exist, downloads the URL into that location. Clips .kml
// files and converts to a .bin.
pub fn download_kml(
    output: &str,
    url: &str,
    bounds: &geom::GPSBounds,
    require_all_pts_in_bounds: bool,
    timer: &mut Timer,
) {
    assert!(url.ends_with(".kml"));
    let output = abstio::path(output);
    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the directory
    std::fs::create_dir_all(Path::new(&output).parent().unwrap())
        .expect("Creating parent dir failed");

    let tmp = "tmp_output";
    if Path::new(&output.replace(".bin", ".kml")).exists() {
        std::fs::copy(output.replace(".bin", ".kml"), tmp).unwrap();
    } else {
        println!("- Missing {}, so downloading {}", output, url);
        must_run_cmd(
            Command::new("curl")
                .arg("--fail")
                .arg("-L")
                .arg("-o")
                .arg(tmp)
                .arg(url),
        );
    }

    println!("- Extracting KML data");

    let shapes = kml::load(tmp, bounds, require_all_pts_in_bounds, timer).unwrap();
    abstio::write_binary(output.clone(), &shapes);
    // Keep the intermediate file; otherwise we inadvertently grab new upstream data when
    // changing some binary formats
    std::fs::rename(tmp, output.replace(".bin", ".kml")).unwrap();
}

// Uses osmconvert to clip the input .osm (or .pbf) against a polygon and produce some output.
// Skips if the output exists.
pub fn osmconvert(
    input: &str,
    clipping_polygon: String,
    output: String,
    config: &ImporterConfiguration,
) {
    let input = abstio::path(input);
    let clipping_polygon = clipping_polygon;
    let output = abstio::path(output);

    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    println!("- Clipping {} to {}", input, clipping_polygon);

    must_run_cmd(
        Command::new(&config.osmconvert)
            .arg(input)
            .arg(format!("-B={}", clipping_polygon))
            .arg("--complete-ways")
            .arg(format!("-o={}", output)),
    );
}

// Converts a RawMap to a Map.
pub fn raw_to_map(
    name: &MapName,
    build_ch: bool,
    keep_bldg_tags: bool,
    timer: &mut Timer,
) -> map_model::Map {
    timer.start(format!("Raw->Map for {}", name.describe()));
    let raw: map_model::raw::RawMap = abstio::read_binary(abstio::path_raw_map(name), timer);
    let map = map_model::Map::create_from_raw(raw, build_ch, keep_bldg_tags, timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    timer.stop(format!("Raw->Map for {}", name.describe()));

    // TODO Just sticking this here for now
    if name.map == "huge_seattle" || name == &MapName::new("leeds", "huge") {
        timer.start("generating city manifest");
        abstio::write_binary(
            abstio::path(format!("system/{}/city.bin", map.get_city_name())),
            &map_model::City::from_huge_map(&map),
        );
        timer.stop("generating city manifest");
    }

    map
}
