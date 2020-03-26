use std::path::Path;
use std::process::Command;

pub fn download(output: &str, url: &str) {
    if Path::new(output).exists() {
        println!("- {} already exists", output);
        return;
    }
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

pub fn rm<I: Into<String>>(path: I) {
    let path = path.into();
    println!("- Removing {}", path);
    run(Command::new("rm").arg("-rfv").arg(path));
}

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
