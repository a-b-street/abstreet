// This crate contains tests for the map importing pipeline. They're closer to integration tests
// than unit tests, mostly because of how tedious and brittle it would be to specify the full input
// to individual pieces of the pipeline.

#[cfg(test)]
mod turns;

// Run the contents of a .osm through the full map importer with default options.
#[cfg(test)]
fn import_map(raw_osm: String) -> map_model::Map {
    let mut timer = abstutil::Timer::new("convert synthetic map");
    let raw = convert_osm::convert(
        convert_osm::Options {
            osm_input: convert_osm::Input::Contents(raw_osm),
            city_name: "oneshot".to_string(),
            name: "test_map".to_string(),
            clip: None,
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
            },
            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(0),
            elevation: None,
            include_railroads: true,
        },
        &mut timer,
    );
    let map = map_model::Map::create_from_raw(raw, true, &mut timer);
    // Useful to debug the result
    if false {
        map.save();
    }
    map
}
