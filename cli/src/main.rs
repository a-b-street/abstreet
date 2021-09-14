// TODO Can we list example invocations in the help text?
//
// dump-json data/system/us/seattle/maps/montlake.bin
//
// dump-json data/system/us/seattle/scenarios/montlake/weekday.bin
//
// random-scenario --map data/system/us/seattle/maps/montlake.bin --rng-seed 42 --scenario-name thursday
//
// augment-scenario --input-scenario=data/system/us/seattle/scenarios/montlake/thursday.bin --add-lunch-trips
//
// clip-osm --pbf-path data/input/us/seattle/osm/washington-latest.osm.pbf --clip-path importer/config/us/seattle/montlake.poly --out-path montlake.osm.xml
//
// geo-json-to-osmosis < boundary.geojson
//
// import-grid2-demand --input=sample.csv --map data/system/us/seattle/maps/montlake.bin

#[macro_use]
extern crate log;

mod augment_scenario;
mod clip_osm;
mod import_grid2demand;

use structopt::StructOpt;

use abstutil::Timer;

#[derive(StructOpt)]
#[structopt(name = "abcli", about = "The A/B Street multi-tool")]
enum Command {
    /// Print a binary map or scenario file as JSON
    DumpJSON {
        #[structopt()]
        path: String,
    },
    /// Generates a random scenario using the proletariat robot travel demand model
    RandomScenario {
        /// A seed for generating random numbers
        #[structopt(long)]
        rng_seed: u64,
        /// The path to a map to generate a scenario for
        #[structopt(long)]
        map: String,
        /// The name of the scenario to generate
        #[structopt(long)]
        scenario_name: String,
    },
    /// Modifies the schedule of every person in an existing scenario.
    AugmentScenario {
        /// The path to a scenario to augment. This will be modified in-place.
        ///
        /// This tool isn't very smart about detecting if a scenario already has these extra trips added
        /// in; be careful about running this on the correct input.
        #[structopt(long)]
        input_scenario: String,
        /// For people with only a single trip, add a return trip back home sometime 4-12 hours
        /// later
        #[structopt(long)]
        add_return_trips: bool,
        /// Before a person's final trp home, insert a round-trip to a nearby cafe or restaurant
        #[structopt(long)]
        add_lunch_trips: bool,
        /// A seed for generating random numbers
        #[structopt(long, default_value = "42")]
        rng_seed: u64,
    },
    /// Clips an OSM file to a boundary. This is a simple Rust port of `osmconvert large_map.osm
    /// -B=clipping.poly --complete-ways -o=smaller_map.osm`.
    ClipOSM {
        /// The path to the input .osm.pbf file
        #[structopt(long)]
        pbf_path: String,
        /// The path to an Osmosis boundary polygon
        #[structopt(long)]
        clip_path: String,
        /// The path to write the XML results
        #[structopt(long)]
        out_path: String,
    },
    /// Reads GeoJSON input from STDIN, extracts a polygon from every feature, and writes numbered
    /// files in the https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format format
    /// as output.
    GeoJSONToOsmosis,
    /// Import a scenario from https://github.com/asu-trans-ai-lab/grid2demand.
    ImportGrid2Demand {
        /// The path to a grid2demand CSV file
        #[structopt(long)]
        input: String,
        /// The path to a map matching the grid2demand data
        #[structopt(long)]
        map: String,
    },
}

fn main() {
    // Short implementations can stay in this file, but please split larger subcommands to their
    // own module.
    match Command::from_args() {
        Command::DumpJSON { path } => dump_json(path),
        Command::RandomScenario {
            rng_seed,
            map,
            scenario_name,
        } => random_scenario(rng_seed, map, scenario_name),
        Command::AugmentScenario {
            input_scenario,
            add_return_trips,
            add_lunch_trips,
            rng_seed,
        } => augment_scenario::run(input_scenario, add_return_trips, add_lunch_trips, rng_seed),
        Command::ClipOSM {
            pbf_path,
            clip_path,
            out_path,
        } => clip_osm::run(pbf_path, clip_path, out_path).unwrap(),
        Command::GeoJSONToOsmosis => geojson_to_osmosis().unwrap(),
        Command::ImportGrid2Demand { input, map } => import_grid2demand::run(input, map).unwrap(),
    }
}

fn dump_json(path: String) {
    if path.contains("/maps/") {
        let map = map_model::Map::load_synchronously(path, &mut Timer::throwaway());
        println!("{}", abstutil::to_json(&map));
    } else if path.contains("/scenarios/") {
        let scenario: sim::Scenario = abstio::read_binary(path, &mut Timer::throwaway());
        println!("{}", abstutil::to_json(&scenario));
    } else {
        panic!(
            "Don't know how to dump JSON for {}. Only maps and scenarios are supported.",
            path
        );
    }
}

fn random_scenario(rng_seed: u64, map: String, scenario_name: String) {
    use rand::SeedableRng;
    use rand_xorshift::XorShiftRng;

    let mut rng = XorShiftRng::seed_from_u64(rng_seed);
    let map = map_model::Map::load_synchronously(map, &mut Timer::throwaway());
    let mut scenario =
        sim::ScenarioGenerator::proletariat_robot(&map, &mut rng, &mut Timer::throwaway());
    scenario.scenario_name = scenario_name;
    scenario.save();
    println!(
        "Wrote {}",
        abstio::path_scenario(&scenario.map_name, &scenario.scenario_name)
    );
}

fn geojson_to_osmosis() -> anyhow::Result<()> {
    use std::io::{self, Read};

    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    for (idx, points) in geom::LonLat::parse_geojson_polygons(buffer)?
        .into_iter()
        .enumerate()
    {
        let path = format!("boundary{}.poly", idx);
        geom::LonLat::write_osmosis_polygon(&path, &points)?;
        println!("Wrote {}", path);
    }
    Ok(())
}
