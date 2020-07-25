use abstutil::Timer;
use std::path::Path;
use std::process::Command;

// If the output file doesn't already exist, downloads the URL into that location. Automatically
// uncompresses .zip and .gz files.
pub fn download(output: &str, url: &str) {
    let output = abstutil::path(output);
    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the directory
    std::fs::create_dir_all(Path::new(&output).parent().unwrap())
        .expect("Creating parent dir failed");

    let tmp = "tmp_output";
    println!("- Missing {}, so downloading {}", output, url);
    run(Command::new("curl")
        .arg("--fail")
        .arg("-L")
        .arg("-o")
        .arg(tmp)
        .arg(url));

    // Argh the Dropbox URL is .zip?dl=0
    if url.contains(".zip") {
        let unzip_to = if output.ends_with("/") {
            output
        } else {
            Path::new(&output).parent().unwrap().display().to_string()
        };
        println!("- Unzipping into {}", unzip_to);
        run(Command::new("unzip").arg(tmp).arg("-d").arg(unzip_to));
        std::fs::remove_file(tmp).unwrap();
    } else if url.ends_with(".gz") {
        println!("- Gunzipping");
        std::fs::rename(tmp, format!("{}.gz", output)).unwrap();
        run(Command::new("gunzip").arg(format!("{}.gz", output)));
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
    let output = abstutil::path(output);
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
        run(Command::new("curl")
            .arg("--fail")
            .arg("-L")
            .arg("-o")
            .arg(tmp)
            .arg(url));
    }

    println!("- Extracting KML data");

    let shapes = kml::load(tmp, bounds, require_all_pts_in_bounds, timer).unwrap();
    abstutil::write_binary(output.clone(), &shapes);
    // Keep the intermediate file; otherwise we inadvertently grab new upstream data when
    // changing some binary formats
    std::fs::rename(tmp, output.replace(".bin", ".kml")).unwrap();
}

// Uses osmconvert to clip the input .osm (or .pbf) against a polygon and produce some output.
// Skips if the output exists.
pub fn osmconvert(input: &str, clipping_polygon: String, output: String) {
    let input = abstutil::path(input);
    let clipping_polygon = abstutil::path(clipping_polygon);
    let output = abstutil::path(output);

    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    println!("- Clipping {} to {}", input, clipping_polygon);

    run(Command::new("osmconvert")
        .arg(input)
        .arg(format!("-B={}", clipping_polygon))
        .arg("--complete-ways")
        .arg(format!("-o={}", output)));
}

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

// Converts a RawMap to a Map.
pub fn raw_to_map(name: &str, build_ch: bool, timer: &mut Timer) -> map_model::Map {
    timer.start(format!("Raw->Map for {}", name));
    let raw: map_model::raw::RawMap = abstutil::read_binary(abstutil::path_raw_map(name), timer);
    let map = map_model::Map::create_from_raw(raw, build_ch, timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    timer.stop(format!("Raw->Map for {}", name));

    // TODO Just sticking this here for now
    if map.get_name() == "huge_seattle" {
        timer.start("generating city manifest");
        abstutil::write_binary(
            abstutil::path(format!("system/cities/{}.bin", map.get_city_name())),
            &map_model::City::new(&map),
        );
        timer.stop("generating city manifest");
    }

    map
}
