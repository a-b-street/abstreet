use crate::utils::{download, osmconvert, rm};

fn input() {
    download(
        "../data/input/barranquilla/osm/colombia.osm.pbf",
        "http://download.geofabrik.de/south-america/colombia-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "../data/input/barranquilla/osm/colombia.osm.pbf",
        format!("../data/input/barranquilla/polygons/{}.poly", name),
        format!("../data/input/barranquilla/osm/{}.osm", name),
    );
    rm(format!("../data/system/maps/{}.bin", name));

    println!("- Running convert_osm");
    let output = format!("../data/input/raw_maps/{}.bin", name);
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: format!("../data/input/barranquilla/osm/{}.osm", name),
            output: output.clone(),
            city_name: "barranquilla".to_string(),
            name: name.to_string(),

            parking_shapes: None,
            public_offstreet_parking: None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::OnePerBldg,
            sidewalks: None,
            gtfs: None,
            elevation: None,
            clip: Some(format!("../data/input/barranquilla/polygons/{}.poly", name)),
            drive_on_right: true,
        },
        &mut abstutil::Timer::throwaway(),
    );
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}
