use abstutil;
use convert_osm;
use runner::TestRunner;

pub fn run(t: &mut TestRunner) {
    t.run_slow(
        "convert_twice",
        Box::new(|_| {
            let flags = convert_osm::Flags {
                osm: "../data/input/montlake.osm".to_string(),
                elevation: "../data/input/N47W122.hgt".to_string(),
                traffic_signals: "../data/input/TrafficSignals.shp".to_string(),
                parcels: "../data/shapes/parcels".to_string(),
                parking_shapes: "../data/shapes/blockface".to_string(),
                gtfs: "../data/input/google_transit_2018_18_08".to_string(),
                neighborhoods: "../data/input/neighborhoods.geojson".to_string(),
                output: "convert_twice".to_string(),
            };

            let map1 = convert_osm::convert(&flags, &mut abstutil::Timer::new("convert map"));
            let map2 = convert_osm::convert(&flags, &mut abstutil::Timer::new("convert map"));

            if map1 != map2 {
                // TODO tmp files
                abstutil::write_json("map1.json", &map1).unwrap();
                abstutil::write_json("map2.json", &map2).unwrap();
                panic!("map1.json and map2.json differ");
            }
        }),
    );
}
