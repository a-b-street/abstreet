use abstio::path_shared_input;
use abstutil::Timer;
use map_model::raw::RawMap;

use crate::configuration::ImporterConfiguration;
use crate::utils::download;

pub fn import_collision_data(map: &RawMap, config: &ImporterConfiguration, timer: &mut Timer) {
    download(
        config,
        path_shared_input("Road Safety Data - Accidents 2019.csv"),
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");

    // Always do this, it's idempotent and fast
    let shapes = kml::ExtraShapes::load_csv(
        path_shared_input("Road Safety Data - Accidents 2019.csv"),
        &map.gps_bounds,
        timer,
    )
    .unwrap();
    let collisions = collisions::import_stats19(
        shapes,
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");
    abstio::write_binary(
        map.get_city_name().input_path("collisions.bin"),
        &collisions,
    );
}
