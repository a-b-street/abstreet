mod austin;
mod barranquilla;
mod los_angeles;
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

    use_fixes: bool,
    only_map: Option<String>,
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

        // By default, use geometry fixes from map_editor.
        use_fixes: !args.enabled("--nofixes"),
        // Only process one map. If not specified, process all maps defined by clipping polygons in
        // data/input/$city/polygons/.
        only_map: args.optional_free(),
    };
    args.done();
    if !job.osm_to_raw && !job.raw_to_map && !job.scenario && !job.scenario_everyone {
        println!(
            "Nothing to do! Pass some combination of --raw, --map, --scenario, --scenario_everyone"
        );
        std::process::exit(1);
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
                "barranquilla" => barranquilla::osm_to_raw(&name),
                "los_angeles" => los_angeles::osm_to_raw(&name),
                "seattle" => seattle::osm_to_raw(&name),
                x => panic!("Unknown city {}", x),
            }
        }

        if job.raw_to_map {
            let thread_name = name.clone();
            //Fetch use_fixes after already loading into job in order to avoid move compile error
            let use_fixes = job.use_fixes;
            let handle = thread::spawn(move || {
                utils::raw_to_map(&thread_name, use_fixes);
            });
            handles.push(handle);
        }

        if job.scenario {
            assert_eq!(job.city, "seattle");
            seattle::ensure_popdat_exists(job.use_fixes);

            let mut timer = abstutil::Timer::new(format!("Scenario for {}", name));
            let map = map_model::Map::new(abstutil::path_map(&name), job.use_fixes, &mut timer);
            soundcast::make_weekday_scenario(&map, &mut timer).save();
        }

        if job.scenario_everyone {
            assert_eq!(job.city, "seattle");
            seattle::ensure_popdat_exists(job.use_fixes);

            let mut timer = abstutil::Timer::new(format!("Scenario for {}", name));
            let map = map_model::Map::new(abstutil::path_map(&name), job.use_fixes, &mut timer);
            soundcast::make_weekday_scenario_with_everyone(&map, &mut timer).save();
        }
    }
    for handle in handles {
        handle.join().unwrap();
    }
}
