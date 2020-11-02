use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

fn input(config: &ImporterConfiguration) {
    download(
        config,
        "input/leeds/osm/west-yorkshire.osm.pbf",
        "https://download.geofabrik.de/europe/great-britain/england/west-yorkshire-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer, config: &ImporterConfiguration) {
    input(config);
    osmconvert(
        "input/leeds/osm/west-yorkshire-latest.osm.pbf",
        format!("input/leeds/polygons/{}.poly", name),
        format!("input/leeds/osm/{}.osm", name),
        config,
    );

    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/leeds/osm/{}.osm", name)),
            city_name: "leeds".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/leeds/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Left,
                bikes_can_use_bus_lanes: false,
                inferred_sidewalks: false,
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
