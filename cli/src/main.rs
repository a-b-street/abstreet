//! A collection of tools, mostly related to importing maps and scenarios. These are bundled as a
//! single executable to reduce the cost of static linking in the release's file size.

#[macro_use]
extern crate log;

mod augment_scenario;
mod clip_osm;
mod generate_houses;
mod geojson_to_osmosis;
mod import_grid2demand;
mod import_scenario;
mod one_step_import;
mod osm2lanes;

use anyhow::Result;
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
    /// Reads a GeoJSON file, extracts a polygon from every feature, and writes numbered files in
    /// the https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format format as
    /// output.
    #[structopt(name = "geojson-to-osmosis")]
    GeoJSONToOsmosis {
        /// The path to a GeoJSON file
        #[structopt()]
        input: String,
    },
    /// Import a scenario from https://github.com/asu-trans-ai-lab/grid2demand.
    ImportGrid2Demand {
        /// The path to a grid2demand CSV file
        #[structopt(long)]
        input: String,
        /// The path to a map matching the grid2demand data
        #[structopt(long)]
        map: String,
    },
    /// Import a JSON scenario in the
    /// https://a-b-street.github.io/docs/tech/dev/formats/scenarios.html format
    ImportScenario {
        /// The path to a JSON scenario file
        #[structopt(long)]
        input: String,
        /// The path to a map matching the scenario data
        #[structopt(long)]
        map: String,
        /// Problems occur when a position is within the map boundary, but not close enough to
        /// buildings. Skip people with problematic positions if true, abort otherwise.
        #[structopt(long)]
        skip_problems: bool,
    },
    /// Transform a JSON map that's been manually edited into the binary format suitable for
    /// simulation.
    ImportJSONMap {
        /// The path to a JSON map file to import
        #[structopt(long)]
        input: String,
        /// The path to write
        #[structopt(long)]
        output: String,
    },
    /// Removes nonessential parts of a Map, for the bike network tool.
    MinifyMap {
        /// The path to a map to shrink. The map is modified in-place.
        #[structopt()]
        map: String,
    },
    /// Procedurally generates houses along empty residential roads of a map
    GenerateHouses {
        /// The path to a map to generate houses for
        #[structopt(long)]
        map: String,
        /// If the tool doesn't generate at least this many houses, then fail. This can be used to
        /// autodetect if a map probably already has most houses tagged in OSM.
        #[structopt(long)]
        num_required: usize,
        /// A seed for generating random numbers
        #[structopt(long, default_value = "42")]
        rng_seed: u64,
        /// The GeoJSON file to write
        #[structopt(long)]
        output: String,
    },
    /// Prints the osm.pbf file from download.geofabrik.de that covers a given boundary.
    ///
    /// This is a useful tool when importing a new map, if you don't already know which geofabrik
    /// file you should use as your OSM input.
    PickGeofabrik {
        /// The path to an [osmosis polygon boundary
        /// file](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format)
        #[structopt()]
        input: String,
    },
    /// Imports a one-shot A/B Street map from a GeoJSON boundary in a single command.
    OneStepImport {
        /// The path to a GeoJSON file with a boundary
        #[structopt(long)]
        geojson_path: String,
        /// What to name the new imported map. The country will always be "zz" (a fake country
        /// code), with the city as "oneshot." This name shouldn't contain spaces or be empty.
        #[structopt(long)]
        map_name: String,
        /// Do people drive on the left side of the road in this map?
        #[structopt(long)]
        drive_on_left: bool,
        /// Use Geofabrik to grab OSM input if true, or Overpass if false. Overpass is faster.
        #[structopt(long)]
        use_geofabrik: bool,
        /// Downgrade crosswalks not matching a `highway=crossing` OSM node into unmarked crossings.
        #[structopt(long)]
        filter_crosswalks: bool,
    },
    /// Imports a one-shot A/B Street map from an .osm file in a single command.
    OneshotImport {
        #[structopt()]
        osm_input: String,
        /// The path to an Osmosis boundary polygon. If omitted, a boundary will be derived from
        /// the .osm file, but borders will likely be broken or missing.
        #[structopt(long)]
        clip_path: Option<String>,
        /// Do people drive on the left side of the road in this map?
        #[structopt(long)]
        drive_on_left: bool,
        /// Downgrade crosswalks not matching a `highway=crossing` OSM node into unmarked crossings.
        #[structopt(long)]
        filter_crosswalks: bool,
        #[structopt(flatten)]
        opts: map_model::RawToMapOptions,
    },
    /// Regenerate all maps and scenarios from scratch.
    RegenerateEverything {
        /// If this command is being run in the cloud, parallelize the jobs by specifying which
        /// shard this invocation should run.
        #[structopt(long, default_value = "0")]
        shard_num: usize,
        /// If this command is being run in the cloud, parallelize the jobs by specifying how many
        /// total shards there are.
        #[structopt(long, default_value = "1")]
        num_shards: usize,
    },
    /// Regenerate all maps from RawMaps in parallel.
    RegenerateAllMaps,
    /// Import RawMaps, maps, scenarios, and city overviews for a single city.
    Import {
        #[structopt(flatten)]
        job: importer::Job,
    },
    /// Generates JSON test cases for osm2lanes.
    #[structopt(name = "osm2lanes")]
    OSM2Lanes {
        /// The path to a map file
        #[structopt()]
        map_path: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cmd = Command::from_args();

    // All but a few commands want logging
    if !matches!(
        cmd,
        Command::DumpJSON { .. } | Command::PickGeofabrik { .. },
    ) {
        abstutil::logger::setup();
    }

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
        } => clip_osm::run(pbf_path, clip_path, out_path)?,
        Command::GeoJSONToOsmosis { input } => geojson_to_osmosis::run(input)?,
        Command::ImportGrid2Demand { input, map } => import_grid2demand::run(input, map)?,
        Command::ImportScenario {
            input,
            map,
            skip_problems,
        } => import_scenario::run(input, map, skip_problems),
        Command::ImportJSONMap { input, output } => import_json_map(input, output),
        Command::MinifyMap { map } => minify_map(map),
        Command::GenerateHouses {
            map,
            num_required,
            rng_seed,
            output,
        } => generate_houses::run(map, num_required, rng_seed, output),
        Command::PickGeofabrik { input } => {
            println!("{}", importer::pick_geofabrik(input).await?)
        }
        Command::OneStepImport {
            geojson_path,
            map_name,
            drive_on_left,
            use_geofabrik,
            filter_crosswalks,
        } => {
            one_step_import::run(
                geojson_path,
                map_name,
                drive_on_left,
                use_geofabrik,
                filter_crosswalks,
            )
            .await?
        }
        Command::OneshotImport {
            osm_input,
            clip_path,
            drive_on_left,
            filter_crosswalks,
            opts,
        } => importer::oneshot(osm_input, clip_path, drive_on_left, filter_crosswalks, opts),
        Command::RegenerateEverything {
            shard_num,
            num_shards,
        } => importer::regenerate_everything(shard_num, num_shards).await,
        Command::RegenerateAllMaps => importer::regenerate_all_maps(),
        Command::Import { job } => job.run(&mut Timer::new("import one city")).await,
        Command::OSM2Lanes { map_path } => osm2lanes::run(map_path),
    }
    Ok(())
}

fn dump_json(path: String) {
    // Just try to deserialize as different formats
    if let Ok(map) =
        abstio::maybe_read_binary::<map_model::Map>(path.clone(), &mut Timer::throwaway())
    {
        println!("{}", abstutil::to_json(&map));
        return;
    }
    if let Ok(scenario) =
        abstio::maybe_read_binary::<sim::Scenario>(path.clone(), &mut Timer::throwaway())
    {
        println!("{}", abstutil::to_json(&scenario));
        return;
    }
    panic!(
        "Don't know how to dump JSON for {}. Only maps and scenarios are supported.",
        path
    );
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

fn import_json_map(input: String, output: String) {
    // TODO This can't handle the output of dump_map! What?!
    let mut map: map_model::Map = abstio::read_json(input, &mut Timer::throwaway());
    map.map_loaded_directly(&mut Timer::throwaway());
    abstio::write_binary(output, &map);
}

fn minify_map(path: String) {
    let mut timer = Timer::new("minify map");
    let mut map = map_model::Map::load_synchronously(path, &mut timer);
    map.minify(&mut timer);
    // This also changes the name, so this won't overwrite anything
    map.save();
}
