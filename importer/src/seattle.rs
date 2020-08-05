use crate::utils::{download, download_kml, osmconvert};
use abstutil::MultiMap;
use geom::{Duration, Time};
use map_model::{BusRouteID, Map};
use serde::Deserialize;
use sim::Scenario;
use std::collections::BTreeMap;
use std::fs::File;

fn input(timer: &mut abstutil::Timer) {
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
        timer,
    );
    // From https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots
    download_kml(
        "input/seattle/offstreet_parking.bin",
        "http://data-seattlecitygis.opendata.arcgis.com/datasets/8e52dfde6d5d45948f7a90654c8d50cd_0.kml",
        &bounds,
        true,
        timer
    );

    download(
        "input/seattle/google_transit/",
        "http://metro.kingcounty.gov/gtfs/google_transit.zip",
    );
}

pub fn osm_to_raw(name: &str, timer: &mut abstutil::Timer) {
    input(timer);
    osmconvert(
        "input/seattle/osm/washington-latest.osm.pbf",
        format!("input/seattle/polygons/{}.poly", name),
        format!("input/seattle/osm/{}.osm", name),
    );

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
            // They mess up 16th and E Marginal badly enough to cause gridlock.
            include_railroads: false,
        },
        timer,
    );
    let output = abstutil::path(format!("input/raw_maps/{}.bin", name));
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
        osm_to_raw("huge_seattle", timer);
    }
    let huge_map = if abstutil::file_exists(abstutil::path_map("huge_seattle")) {
        map_model::Map::new(abstutil::path_map("huge_seattle"), timer)
    } else {
        crate::utils::raw_to_map("huge_seattle", true, timer)
    };

    (crate::soundcast::import_data(&huge_map, timer), huge_map)
}

pub fn adjust_private_parking(map: &mut Map, scenario: &Scenario) {
    for (b, count) in scenario.count_parked_cars_per_bldg().consume() {
        map.hack_override_offstreet_spots_individ(b, count);
    }
    map.save();
}

// This import from GTFS:
// - is specific to Seattle, whose files don't seem to match https://developers.google.com/transit/gtfs/reference
// - is probably wrong
pub fn add_gtfs_schedules(map: &mut Map) {
    // https://www.openstreetmap.org/relation/8616968 as an example, mapping to
    // https://kingcounty.gov/depts/transportation/metro/schedules-maps/route/048.aspx

    let mut trip_marker_to_route: BTreeMap<String, BusRouteID> = BTreeMap::new();
    for br in map.all_bus_routes() {
        if let Some(ref m) = br.gtfs_trip_marker {
            // Dunno what the :0 thing is
            trip_marker_to_route.insert(m.split(":").next().unwrap().to_string(), br.id);
        }
    }

    // Each route has a bunch of trips throughout the day
    let mut trip_marker_to_trips: MultiMap<String, String> = MultiMap::new();
    for rec in
        csv::Reader::from_reader(File::open("data/input/seattle/google_transit/trips.txt").unwrap())
            .deserialize()
    {
        let rec: TripRecord = rec.unwrap();
        if trip_marker_to_route.contains_key(&rec.shape_id) {
            trip_marker_to_trips.insert(rec.shape_id, rec.trip_id);
        }
    }

    // For every trip, find the earliest arrival time. That should be the spawn time.
    let mut trip_to_earliest_time: BTreeMap<String, Time> = BTreeMap::new();
    for rec in csv::Reader::from_reader(
        File::open("data/input/seattle/google_transit/stop_times.txt").unwrap(),
    )
    .deserialize()
    {
        let rec: StopTimeRecord = rec.unwrap();
        let mut time = Time::parse(&rec.arrival_time).unwrap();
        // Maybe we should duplicate these to handle beginning and end of the simulation
        if time > Time::START_OF_DAY + Duration::hours(24) {
            time = time - Duration::hours(24);
        }
        if trip_to_earliest_time
            .get(&rec.trip_id)
            .map(|t| time < *t)
            .unwrap_or(true)
        {
            trip_to_earliest_time.insert(rec.trip_id, time);
        }
    }

    // Collect the spawn times per route
    for (marker, trips) in trip_marker_to_trips.consume() {
        let mut times = Vec::new();
        for trip_id in trips {
            times.push(trip_to_earliest_time.remove(&trip_id).unwrap());
        }
        times.sort();
        times.dedup();

        let br = trip_marker_to_route.remove(&marker).unwrap();
        map.hack_override_orig_spawn_times(br, times);
    }
    map.save();
}

#[derive(Debug, Deserialize)]
struct TripRecord {
    shape_id: String,
    trip_id: String,
}

#[derive(Debug, Deserialize)]
struct StopTimeRecord {
    trip_id: String,
    arrival_time: String,
}
