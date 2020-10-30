use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/tel_aviv/osm/israel-and-palestine-latest.osm.pbf",
        "http://download.geofabrik.de/asia/israel-and-palestine-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer, config: &ImporterConfiguration) {
    input(config);
    osmconvert(
        "input/tel_aviv/osm/israel-and-palestine-latest.osm.pbf",
        format!("input/tel_aviv/polygons/{}.poly", name),
        format!("input/tel_aviv/osm/{}.osm", name),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/tel_aviv/osm/{}.osm", name)),
            city_name: "tel_aviv".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/tel_aviv/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::SomeAdditionalWhereNoData { pct: 50 },
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(10),
            elevation: None,
            include_railroads: true,
        },
        timer,
    );
    map.save();
}
