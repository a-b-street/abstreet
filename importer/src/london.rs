use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/london/osm/greater-london-latest.osm.pbf",
        "http://download.geofabrik.de/europe/great-britain/england/greater-london-latest.osm.pbf",
    );
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
            city_name: "london".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/london/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Left,
                bikes_can_use_bus_lanes: true,
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
}
