use crate::utils::{download, download_kml, osmconvert};
use map_model::Map;
use sim::Scenario;

fn input() {
    download(
        "input/seattle/N47W122.hgt",
        "https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip",
    );
    download(
        "input/seattle/osm/washington-latest.osm.pbf",
        "http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf",
    );
    // Soundcast data comes from https://github.com/psrc/soundcast/releases
    download(
        "input/seattle/parcels_urbansim.txt",
        "https://www.dropbox.com/s/t9oug9lwhdwfc04/psrc_2014.zip?dl=0",
    );

    let bounds = geom::GPSBounds::from(
        geom::LonLat::read_osmosis_polygon(abstutil::path(
            "input/seattle/polygons/huge_seattle.poly",
        ))
        .unwrap(),
    );
    // From http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
    download_kml(
        "input/seattle/blockface.bin",
        "https://opendata.arcgis.com/datasets/a1458ad1abca41869b81f7c0db0cd777_0.kml",
        &bounds,
        true,
    );
    // From https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots
    download_kml(
        "input/seattle/offstreet_parking.bin",
        "http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml",
        &bounds,
        true,
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "input/seattle/osm/washington-latest.osm.pbf",
        format!("input/seattle/polygons/{}.poly", name),
        format!("input/seattle/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/seattle/osm/{}.osm", name)),
            city_name: "seattle".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/seattle/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::raw::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::Blockface(abstutil::path(
                "input/seattle/blockface.bin",
            )),
            public_offstreet_parking: convert_osm::PublicOffstreetParking::GIS(abstutil::path(
                "input/seattle/offstreet_parking.bin",
            )),
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(
                // TODO Utter guesses
                match name {
                    "downtown" => 5,
                    "lakeslice" => 3,
                    "south_seattle" => 5,
                    "udistrict" => 5,
                    _ => 1,
                },
            ),
            elevation: Some(abstutil::path("input/seattle/N47W122.hgt")),
        },
        &mut abstutil::Timer::throwaway(),
    );
    let output = abstutil::path(format!("input/raw_maps/{}.bin", name));
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

pub fn adjust_private_parking(map: &mut Map, scenario: &Scenario) {
    for (b, count) in scenario.count_parked_cars_per_bldg().consume() {
        map.hack_override_offstreet_spots_individ(b, count);
    }
    map.save();
}
