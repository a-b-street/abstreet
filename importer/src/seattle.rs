use crate::utils::{download, osmconvert, rm};

// Grab all raw input files for Seattle.
fn seattle_input() {
    download(
        "../data/input/google_transit/",
        "https://metro.kingcounty.gov/GTFS/google_transit.zip",
    );
    // Like https://data.seattle.gov/dataset/Neighborhoods/2mbt-aqqx, but in GeoJSON, not SHP
    download("../data/input/neighborhoods.geojson", "https://github.com/seattleio/seattle-boundaries-data/raw/master/data/neighborhoods.geojson");
    download(
        "../data/input/N47W122.hgt",
        "https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip",
    );
    download(
        "../data/input/osm/Seattle.osm",
        "http://download.bbbike.org/osm/bbbike/Seattle/Seattle.osm.gz",
    );
    // Soundcast data comes from https://github.com/psrc/soundcast/releases
    download(
        "../data/input/parcels_urbansim.txt",
        "https://www.dropbox.com/s/t9oug9lwhdwfc04/psrc_2014.zip?dl=0",
    );

    // From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
    download(
        "../data/input/blockface.bin",
        "https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml",
    );
    // From https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
    download(
        "../data/input/sidewalks.bin",
        "https://opendata.arcgis.com/datasets/ee6d0642d2a04e35892d0eab77d971d6_2.kml",
    );
    // From https://data.seattle.gov/Transportation/Public-Garages-or-Parking-Lots/xefx-khzm
    download("../data/input/offstreet_parking.bin", "http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml");
}

// Raw input -> RawMap for Seattle
pub fn osm_to_raw(name: &str) {
    seattle_input();
    osmconvert(
        "../data/input/osm/Seattle.osm",
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
            parking_shapes: Some("../data/input/blockface.bin".to_string()),
            public_offstreet_parking: Some("../data/input/offstreet_parking.bin".to_string()),
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::OnePerBldg,
            // TODO These're buggy.
            sidewalks: None,
            gtfs: Some("../data/input/google_transit".to_string()),
            neighborhoods: Some("../data/input/neighborhoods.geojson".to_string()),
            elevation: Some("../data/input/N47W122.hgt".to_string()),
            clip: Some(format!("../data/input/polygons/{}.poly", name)),
            drive_on_right: true,
            output: output.clone(),
        },
        &mut abstutil::Timer::throwaway(),
    );
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}

// Download and pre-process data needed to generate Seattle scenarios.
pub fn ensure_popdat_exists(use_fixes: bool) {
    if abstutil::file_exists(abstutil::path_popdat()) {
        println!("- {} exists, not regenerating it", abstutil::path_popdat());
        return;
    }

    if !abstutil::file_exists(abstutil::path_raw_map("huge_seattle")) {
        osm_to_raw("huge_seattle");
    }
    if !abstutil::file_exists(abstutil::path_map("huge_seattle")) {
        crate::utils::raw_to_map("huge_seattle", use_fixes);
    }

    crate::soundcast::import_data();
}
