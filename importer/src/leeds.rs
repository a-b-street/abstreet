use abstutil::MapName;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/leeds/osm/west-yorkshire.osm.pbf",
        "https://download.geofabrik.de/europe/great-britain/england/west-yorkshire-latest.osm.pbf",
    );

    download(
        config,
        "input/leeds/Road Safety Data - Accidents 2019.csv",
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer, config: &ImporterConfiguration) {
    input(config);
    osmconvert(
        "input/leeds/osm/west-yorkshire.osm.pbf",
        format!("importer/config/leeds/{}.poly", name),
        format!("input/leeds/osm/{}.osm", name),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/leeds/osm/{}.osm", name)),
            name: MapName::new("leeds", name),

            clip: Some(format!("importer/config/leeds/{}.poly", name)),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Left,
                bikes_can_use_bus_lanes: false,
                inferred_sidewalks: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(3),
            elevation: None,
            include_railroads: true,
        },
        timer,
    );
    map.save();

    // Always do this, it's idempotent and fast
    let shapes = kml::ExtraShapes::load_csv(
        "data/input/leeds/Road Safety Data - Accidents 2019.csv",
        &map.gps_bounds,
        timer,
    )
    .unwrap();
    let collisions = collisions::import_stats19(
        shapes,
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");
    abstutil::write_binary("data/input/leeds/collisions.bin".to_string(), &collisions);
}
