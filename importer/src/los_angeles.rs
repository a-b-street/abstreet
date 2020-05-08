use crate::utils::{download, osmconvert};

fn input() {
    download(
        "../data/input/los_angeles/osm/socal.osm.pbf",
        "http://download.geofabrik.de/north-america/us/california/socal-latest.osm.pbf",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "../data/input/los_angeles/osm/socal.osm.pbf",
        format!("../data/input/los_angeles/polygons/{}.poly", name),
        format!("../data/input/los_angeles/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
    let output = format!("../data/input/raw_maps/{}.bin", name);
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: format!("../data/input/los_angeles/osm/{}.osm", name),
            output: output.clone(),
            city_name: "los_angeles".to_string(),
            name: name.to_string(),

            parking_shapes: None,
            public_offstreet_parking: None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            sidewalks: None,
            gtfs: None,
            elevation: None,
            clip: Some(format!("../data/input/los_angeles/polygons/{}.poly", name)),
            drive_on_right: true,
        },
        &mut abstutil::Timer::throwaway(),
    );
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}
