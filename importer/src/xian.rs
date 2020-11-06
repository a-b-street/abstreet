use abstutil::MapName;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/xian/osm/china-latest.osm.pbf",
        "http://download.geofabrik.de/asia/china-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer, config: &ImporterConfiguration) {
    input(config);
    osmconvert(
        "input/xian/osm/china-latest.osm.pbf",
        format!("importer/config/xian/{}.poly", name),
        format!("input/xian/osm/{}.osm", name),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/xian/osm/{}.osm", name)),
            name: MapName::new("xian", name),

            clip: Some(format!("importer/config/xian/{}.poly", name)),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
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
}
