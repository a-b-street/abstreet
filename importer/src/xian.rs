use crate::utils::{download, osmconvert};

fn input() {
    download(
        "input/xian/osm/china-latest.osm.pbf",
        "http://download.geofabrik.de/asia/china-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer) {
    input();
    osmconvert(
        "input/xian/osm/china-latest.osm.pbf",
        format!("input/xian/polygons/{}.poly", name),
        format!("input/xian/osm/{}.osm", name),
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/xian/osm/{}.osm", name)),
            city_name: "xian".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!("input/xian/polygons/{}.poly", name))),
            map_config: map_model::MapConfig {
                driving_side: map_model::raw::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(3),
            elevation: None,
            include_railroads: true,
        },
        timer,
    );
    let output = abstutil::path(format!("input/raw_maps/{}.bin", name));
    abstutil::write_binary(output, &map);
}
