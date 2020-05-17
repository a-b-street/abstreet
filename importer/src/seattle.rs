use crate::utils::{download, osmconvert};

fn input() {
    download(
        "../data/input/seattle/google_transit/",
        "https://metro.kingcounty.gov/GTFS/google_transit.zip",
    );
    download(
        "../data/input/seattle/N47W122.hgt",
        "https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip",
    );
    download(
        "../data/input/seattle/osm/washington-latest.osm.pbf",
        "http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf",
    );
    // Soundcast data comes from https://github.com/psrc/soundcast/releases
    download(
        "../data/input/seattle/parcels_urbansim.txt",
        "https://www.dropbox.com/s/t9oug9lwhdwfc04/psrc_2014.zip?dl=0",
    );

    // From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
    download(
        "../data/input/seattle/blockface.bin",
        "https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml",
    );
    // From https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
    download(
        "../data/input/seattle/sidewalks.bin",
        "https://opendata.arcgis.com/datasets/ee6d0642d2a04e35892d0eab77d971d6_2.kml",
    );
    // From https://data.seattle.gov/Transportation/Public-Garages-or-Parking-Lots/xefx-khzm
    download("../data/input/seattle/offstreet_parking.bin", "http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml");
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "../data/input/seattle/osm/washington-latest.osm.pbf",
        format!("../data/input/seattle/polygons/{}.poly", name),
        format!("../data/input/seattle/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: format!("../data/input/seattle/osm/{}.osm", name),
            city_name: "seattle".to_string(),
            name: name.to_string(),

            parking_shapes: Some("../data/input/seattle/blockface.bin".to_string()),
            public_offstreet_parking: Some(
                "../data/input/seattle/offstreet_parking.bin".to_string(),
            ),
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(
                if name == "udistrict" { 5 } else { 1 },
            ),
            // TODO These're buggy.
            sidewalks: None,
            gtfs: Some("../data/input/seattle/google_transit".to_string()),
            elevation: Some("../data/input/seattle/N47W122.hgt".to_string()),
            clip: Some(format!("../data/input/seattle/polygons/{}.poly", name)),
            drive_on_right: true,
        },
        &mut abstutil::Timer::throwaway(),
    );
    let output = format!("../data/input/raw_maps/{}.bin", name);
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}

// Download and pre-process data needed to generate Seattle scenarios.
#[cfg(feature = "scenarios")]
pub fn ensure_popdat_exists(
    timer: &mut abstutil::Timer,
) -> (crate::soundcast::PopDat, map_model::Map) {
    if abstutil::file_exists(abstutil::path_popdat()) {
        println!("- {} exists, not regenerating it", abstutil::path_popdat());
        return (
            abstutil::read_binary(abstutil::path_popdat(), timer),
            map_model::Map::new(abstutil::path_map("huge_seattle"), timer),
        );
    }

    if !abstutil::file_exists(abstutil::path_raw_map("huge_seattle")) {
        osm_to_raw("huge_seattle");
    }
    let huge_map = if abstutil::file_exists(abstutil::path_map("huge_seattle")) {
        map_model::Map::new(abstutil::path_map("huge_seattle"), timer)
    } else {
        crate::utils::raw_to_map("huge_seattle", true, timer)
    };

    (crate::soundcast::import_data(&huge_map), huge_map)
}
