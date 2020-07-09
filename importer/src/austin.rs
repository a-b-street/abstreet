use crate::utils::{download, osmconvert};

fn input() {
    download(
        "input/austin/osm/Austin.osm",
        "https://download.bbbike.org/osm/bbbike/Austin/Austin.osm.gz",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "input/austin/osm/Austin.osm",
        format!("input/austin/polygons/{}.poly", name),
        format!("input/austin/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/austin/osm/{}.osm", name)),
            city_name: "austin".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/austin/polygons/{}.poly",
                name
            ))),
            drive_on_right: true,

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
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
