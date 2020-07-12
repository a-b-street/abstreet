use crate::utils::{download, osmconvert};

fn input() {
    download(
        "input/krakow/osm/malopolskie-latest.osm.pbf",
        "http://download.geofabrik.de/europe/poland/malopolskie-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "input/krakow/osm/malopolskie-latest.osm.pbf",
        format!("input/krakow/polygons/{}.poly", name),
        format!("input/krakow/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
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
                driving_side: map_model::raw::DrivingSide::Right,
                bikes_can_use_bus_lanes: false,
            },

            onstreet_parking: convert_osm::OnstreetParking::SomeResidential { pct: 50 },
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            elevation: None,
        },
        &mut abstutil::Timer::throwaway(),
    );
    let output = abstutil::path(format!("input/raw_maps/{}.bin", name));
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}
