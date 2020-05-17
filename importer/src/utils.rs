use abstutil::Timer;
use std::path::Path;
use std::process::Command;

// If the output file doesn't already exist, downloads the URL into that location. Automatically
// uncompresses .zip and .gz files. .kml files are automatically clipped to a hardcoded boundary of
// Seattle.
pub fn download(output: &str, url: &str) {
    if Path::new(output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the directory
    std::fs::create_dir_all(Path::new(output).parent().unwrap())
        .expect("Creating parent dir failed");

    let tmp = "tmp_output";
    if url.ends_with(".kml") && Path::new(&output.replace(".bin", ".kml")).exists() {
        run(Command::new("cp")
            .arg(output.replace(".bin", ".kml"))
            .arg(tmp));
    } else {
        println!("- Missing {}, so downloading {}", output, url);
        run(Command::new("curl")
            .arg("--fail")
            .arg("-L")
            .arg("-o")
            .arg(tmp)
            .arg(url));
    }

    // Argh the Dropbox URL is .zip?dl=0
    if url.contains(".zip") {
        let unzip_to = if output.ends_with("/") {
            output.to_string()
        } else {
            Path::new(output).parent().unwrap().display().to_string()
        };
        println!("- Unzipping into {}", unzip_to);
        run(Command::new("unzip").arg(tmp).arg("-d").arg(unzip_to));
        rm(tmp);
    } else if url.ends_with(".gz") {
        println!("- Gunzipping");
        run(Command::new("mv").arg(tmp).arg(format!("{}.gz", output)));
        run(Command::new("gunzip").arg(format!("{}.gz", output)));
    } else if url.ends_with(".kml") {
        println!("- Extracting KML data");

        let shapes = kml::load(
            tmp,
            &geom::GPSBounds::seattle_bounds(),
            &mut abstutil::Timer::new("extracting shapes from KML"),
        )
        .unwrap();
        abstutil::write_binary(output.to_string(), &shapes);
        // Keep the intermediate file; otherwise we inadvertently grab new upstream data when
        // changing some binary formats
        run(Command::new("mv")
            .arg(tmp)
            .arg(output.replace(".bin", ".kml")));
    } else {
        run(Command::new("mv").arg(tmp).arg(output));
    }
}

// Uses osmconvert to clip the input .osm (or .pbf) against a polygon and produce some output.
// Skips if the output exists.
pub fn osmconvert(input: &str, clipping_polygon: String, output: String) {
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

// Removes files. Be careful!
pub fn rm<I: Into<String>>(path: I) {
    let path = path.into();
    println!("- Removing {}", path);
    run(Command::new("rm").arg("-rfv").arg(path));
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
    map
}
