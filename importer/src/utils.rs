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

    println!("- Missing {}, so downloading {}", output, url);
    let tmp = "tmp_output";
    run(Command::new("curl")
        .arg("--fail")
        .arg("-L")
        .arg("-o")
        .arg(tmp)
        .arg(url));

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
        rm(tmp);
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
pub fn raw_to_map(name: &str, use_fixes: bool) {
    let mut timer = abstutil::Timer::new(format!("Raw->Map for {}", name));
    let map = map_model::Map::new(abstutil::path_raw_map(name), use_fixes, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
}
