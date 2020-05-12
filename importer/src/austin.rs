use crate::utils::{download, osmconvert};

fn input() {
    download(
        "../data/input/austin/osm/Austin.osm",
        "https://download.bbbike.org/osm/bbbike/Austin/Austin.osm.gz",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "../data/input/austin/osm/Austin.osm",
        format!("../data/input/austin/polygons/{}.poly", name),
        format!("../data/input/austin/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: format!("../data/input/austin/osm/{}.osm", name),
            city_name: "austin".to_string(),
            name: name.to_string(),

            parking_shapes: None,
            public_offstreet_parking: None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            sidewalks: None,
            gtfs: None,
            elevation: None,
            clip: Some(format!("../data/input/austin/polygons/{}.poly", name)),
            drive_on_right: true,
        },
        &mut abstutil::Timer::throwaway(),
    );
    let output = format!("../data/input/raw_maps/{}.bin", name);
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}
