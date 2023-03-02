use std::path::Path;
use std::process::Command;

use abstio::{CityName, MapName};
use abstutil::{must_run_cmd, Timer};
use map_model::RawToMapOptions;
use raw_map::RawMap;

use crate::configuration::ImporterConfiguration;

/// If the output file doesn't already exist, downloads the URL into that location. Automatically
/// uncompresses .zip and .gz files. Assumes a proper path is passed in (including data/).
pub async fn download(config: &ImporterConfiguration, output: String, url: &str) {
    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the directory
    fs_err::create_dir_all(Path::new(&output).parent().unwrap())
        .expect("Creating parent dir failed");

    let tmp_file = format!("{output}_TMP");
    let tmp = &tmp_file;
    println!("- Missing {}, so downloading {}", output, url);
    abstio::download_to_file(url, None, tmp).await.unwrap();

    if url.contains(".zip") {
        let unzip_to = if output.ends_with('/') {
            output
        } else {
            // TODO In this case, there's no guarantee there's only one unzipped file and the name
            // matches!
            Path::new(&output).parent().unwrap().display().to_string()
        };
        println!("- Unzipping into {}", unzip_to);
        must_run_cmd(Command::new(&config.unzip).arg(tmp).arg("-d").arg(unzip_to));
        fs_err::remove_file(tmp).unwrap();
    } else if url.contains(".gz") {
        println!("- Gunzipping");
        fs_err::rename(tmp, format!("{}.gz", output)).unwrap();

        let mut gunzip_cmd = Command::new(&config.gunzip);
        for arg in config.gunzip_args.split_ascii_whitespace() {
            gunzip_cmd.arg(arg);
        }
        must_run_cmd(gunzip_cmd.arg(format!("{}.gz", output)));
    } else {
        fs_err::rename(tmp, output).unwrap();
    }
}

/// If the output file doesn't already exist, downloads the URL into that location. Clips .kml
/// files and converts to a .bin.
pub async fn download_kml(
    output: String,
    url: &str,
    bounds: &geom::GPSBounds,
    require_all_pts_in_bounds: bool,
    timer: &mut Timer<'_>,
) {
    assert!(url.ends_with(".kml"));
    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the directory
    fs_err::create_dir_all(Path::new(&output).parent().unwrap())
        .expect("Creating parent dir failed");

    let tmp_file = format!("{output}_TMP");
    let tmp = &tmp_file;
    if Path::new(&output.replace(".bin", ".kml")).exists() {
        fs_err::copy(output.replace(".bin", ".kml"), tmp).unwrap();
    } else {
        println!("- Missing {}, so downloading {}", output, url);
        abstio::download_to_file(url, None, tmp).await.unwrap();
    }

    println!("- Extracting KML data");

    let shapes = kml::load(tmp.to_string(), bounds, require_all_pts_in_bounds, timer).unwrap();
    abstio::write_binary(output.clone(), &shapes);
    // Keep the intermediate file; otherwise we inadvertently grab new upstream data when
    // changing some binary formats
    fs_err::rename(tmp, output.replace(".bin", ".kml")).unwrap();
}

/// Uses osmium to clip the input .osm (or .pbf) against a polygon and produce some output.  Skips
/// if the output exists.
pub fn osmium(
    input: String,
    clipping_polygon: String,
    output: String,
    config: &ImporterConfiguration,
) {
    if Path::new(&output).exists() {
        println!("- {} already exists", output);
        return;
    }
    // Create the output directory if needed
    fs_err::create_dir_all(Path::new(&output).parent().unwrap())
        .expect("Creating parent dir failed");

    println!("- Clipping {} to {}", input, clipping_polygon);

    // --strategy complete_ways is default
    must_run_cmd(
        Command::new(&config.osmium)
            .arg("extract")
            .arg("-p")
            .arg(clipping_polygon)
            .arg(input)
            .arg("-o")
            .arg(output)
            .arg("-f")
            // Smaller files without author, timestamp, version
            .arg("osm,add_metadata=false"),
    );
}

/// Creates a RawMap from OSM and other input data.
pub async fn osm_to_raw(
    name: MapName,
    timer: &mut abstutil::Timer<'_>,
    config: &ImporterConfiguration,
) -> RawMap {
    if name.city == CityName::seattle() {
        crate::seattle::input(config, timer).await;
    }
    let opts = crate::map_config::config_for_map(&name);
    if let Some(ref url) = opts.gtfs_url {
        download(config, name.city.input_path("gtfs/"), url).await;
    }

    let boundary_polygon = format!(
        "importer/config/{}/{}/{}.geojson",
        name.city.country, name.city.city, name.map
    );
    let (osm_url, local_osm_file) = crate::pick_geofabrik(boundary_polygon.clone())
        .await
        .unwrap();
    download(config, local_osm_file.clone(), &osm_url).await;

    osmium(
        local_osm_file,
        boundary_polygon.clone(),
        name.city.input_path(format!("osm/{}.osm", name.map)),
        config,
    );

    let map = convert_osm::convert(
        name.city.input_path(format!("osm/{}.osm", name.map)),
        name.clone(),
        Some(boundary_polygon),
        opts,
        timer,
    );
    map.save();
    map
}

/// Converts a RawMap to a Map.
pub fn raw_to_map(name: &MapName, opts: RawToMapOptions, timer: &mut Timer) -> map_model::Map {
    timer.start(format!("Raw->Map for {}", name.describe()));
    let raw: RawMap = abstio::read_binary(abstio::path_raw_map(name), timer);
    let map = map_model::Map::create_from_raw(raw, opts, timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    timer.stop(format!("Raw->Map for {}", name.describe()));

    // TODO Just sticking this here for now
    if name.map == "huge_seattle" || name == &MapName::new("gb", "leeds", "huge") {
        timer.start("generating city manifest");
        abstio::write_binary(
            abstio::path(format!(
                "system/{}/{}/city.bin",
                map.get_city_name().country,
                map.get_city_name().city
            )),
            &map_model::City::from_huge_map(&map),
        );
        timer.stop("generating city manifest");
    }

    map
}
