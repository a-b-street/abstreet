use crate::utils::{download, osmconvert, rm};

fn input() {
    download(
        "../data/input/osm/Austin.osm",
        "https://download.bbbike.org/osm/bbbike/Austin/Austin.osm.gz",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "../data/input/osm/Austin.osm",
        format!("../data/input/polygons/{}.poly", name),
        format!("../data/input/osm/{}.osm", name),
    );
    rm(format!("../data/input/neighborhoods/{}", name));
    rm(format!("../data/system/maps/{}.bin", name));

    println!("- Running convert_osm");
    let output = format!("../data/input/raw_maps/{}.bin", name);
    let map = convert_osm::convert(
        convert_osm::Options {
            osm: format!("../data/input/osm/{}.osm", name),
            parking_shapes: None,
            public_offstreet_parking: None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::OnePerBldg,
            sidewalks: None,
            gtfs: None,
            neighborhoods: None,
            elevation: None,
            clip: Some(format!("../data/input/polygons/{}.poly", name)),
            drive_on_right: true,
            output: output.clone(),
        },
        &mut abstutil::Timer::throwaway(),
    );
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}
