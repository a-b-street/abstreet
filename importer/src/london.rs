use abstutil::MapName;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/london/osm/greater-london-latest.osm.pbf",
        "http://download.geofabrik.de/europe/great-britain/england/greater-london-latest.osm.pbf",
    );

    download(
        config,
        "input/london/Road Safety Data - Accidents 2019.csv",
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer, config: &ImporterConfiguration) {
    input(config);
    osmconvert(
        "input/london/osm/greater-london-latest.osm.pbf",
        format!("input/london/polygons/{}.poly", name),
        format!("input/london/osm/{}.osm", name),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/london/osm/{}.osm", name)),
            name: MapName::new("london", name),

            clip: Some(abstutil::path(format!(
                "input/london/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Left,
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(10),
            elevation: None,
            include_railroads: true,
        },
        timer,
    );
    map.save();

    // Always do this, it's idempotent and fast
    let shapes = kml::ExtraShapes::load_csv(
        "data/input/london/Road Safety Data - Accidents 2019.csv",
        &map.gps_bounds,
        timer,
    )
    .unwrap();
    let collisions = collisions::import_stats19(
        shapes,
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");
    abstutil::write_binary("data/input/london/collisions.bin".to_string(), &collisions);
}
