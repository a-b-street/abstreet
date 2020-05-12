mod austin;
mod seattle;
mod soundcast;
mod utils;

use std::thread;

struct Job {
    city: String,
    osm_to_raw: bool,
    raw_to_map: bool,
    scenario: bool,
    scenario_everyone: bool,

    only_map: Option<String>,

    oneshot: Option<String>,
}

fn main() {
    let mut args = abstutil::CmdArgs::new();
    let job = Job {
        city: args.optional("--city").unwrap_or("seattle".to_string()),
        // Download all raw input files, then convert OSM to the intermediate RawMap.
        osm_to_raw: args.enabled("--raw"),
        // Convert the RawMap to the final Map format.
        raw_to_map: args.enabled("--map"),
        // Download trip demand data, then produce the typical weekday scenario.
        scenario: args.enabled("--scenario"),
        // Produce a variation of the weekday scenario including off-map trips.
        scenario_everyone: args.enabled("--scenario_everyone"),

        // Only process one map. If not specified, process all maps defined by clipping polygons in
        // data/input/$city/polygons/.
        only_map: args.optional_free(),

        // Ignore other arguments and just convert the given .osm file to a Map.
        oneshot: args.optional("--oneshot"),
    };
    args.done();
    if !job.osm_to_raw
        && !job.raw_to_map
        && !job.scenario
        && !job.scenario_everyone
        && job.oneshot.is_none()
    {
        println!(
            "Nothing to do! Pass some combination of --raw, --map, --scenario, \
             --scenario_everyone or --oneshot"
        );
        std::process::exit(1);
    }

    if let Some(path) = job.oneshot {
        oneshot(path);
        return;
    }

    let names = if let Some(n) = job.only_map {
        println!("- Just working on {}", n);
        vec![n]
    } else {
        println!("- Working on all {} maps", job.city);
        abstutil::list_all_objects(format!("../data/input/{}/polygons", job.city))
    };

    let mut handles = vec![];
    for name in names {
        if job.osm_to_raw {
            match job.city.as_ref() {
                "austin" => austin::osm_to_raw(&name),
                "seattle" => seattle::osm_to_raw(&name),
                x => panic!("Unknown city {}", x),
            }
        }

        if job.raw_to_map {
            let name = name.clone();
            let handle = thread::spawn(move || {
                utils::raw_to_map(&name);
            });
            handles.push(handle);
            // TODO Bug: if regenerating map and scenario at the same time, this doesn't work.
        }

        if job.scenario {
            assert_eq!(job.city, "seattle");
            seattle::ensure_popdat_exists();

            let mut timer = abstutil::Timer::new(format!("Scenario for {}", name));
            let map = map_model::Map::new(abstutil::path_map(&name), &mut timer);
            soundcast::make_weekday_scenario(&map, &mut timer).save();
        }

        if job.scenario_everyone {
            assert_eq!(job.city, "seattle");
            seattle::ensure_popdat_exists();

            let mut timer = abstutil::Timer::new(format!("Scenario for {}", name));
            let map = map_model::Map::new(abstutil::path_map(&name), &mut timer);
            soundcast::make_weekday_scenario_with_everyone(&map, &mut timer).save();
        }
    }
    for handle in handles {
        handle.join().unwrap();
    }
}

fn oneshot(osm_path: String) {
    let mut timer = abstutil::Timer::new("oneshot");
    println!("- Running convert_osm on {}", osm_path);
    let name = abstutil::basename(&osm_path);
    let raw = convert_osm::convert(
        convert_osm::Options {
            osm_input: osm_path,
            city_name: "oneshot".to_string(),
            name: name.clone(),

            parking_shapes: None,
            public_offstreet_parking: None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            sidewalks: None,
            gtfs: None,
            elevation: None,
            clip: None,
            drive_on_right: true,
        },
        &mut timer,
    );
    let map = map_model::Map::create_from_raw(raw, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    println!("{} has been created", abstutil::path_map(&name));
}
