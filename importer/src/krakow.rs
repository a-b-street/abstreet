use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/krakow/osm/malopolskie-latest.osm.pbf",
        "http://download.geofabrik.de/europe/poland/malopolskie-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer, config: &ImporterConfiguration) {
    input(config);
    osmconvert(
        "input/krakow/osm/malopolskie-latest.osm.pbf",
        format!("input/krakow/polygons/{}.poly", name),
        format!("input/krakow/osm/{}.osm", name),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/krakow/osm/{}.osm", name)),
            city_name: "krakow".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/krakow/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Right,
                bikes_can_use_bus_lanes: false,
            },

            onstreet_parking: convert_osm::OnstreetParking::SomeAdditionalWhereNoData { pct: 90 },
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(3), /* TODO: support amenity=parking_entrance */
            // TODO: investigate why some many buildings drop their private parkings
            elevation: None,
            include_railroads: true,
        },
        timer,
    );
    map.save();
}
