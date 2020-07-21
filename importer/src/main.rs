mod berlin;
mod krakow;
mod seattle;
#[cfg(feature = "scenarios")]
mod soundcast;
mod utils;

// TODO Might be cleaner to express as a dependency graph?

struct Job {
    city: String,
    osm_to_raw: bool,
    raw_to_map: bool,
    scenario: bool,
    scenario_everyone: bool,

    skip_ch: bool,

    only_map: Option<String>,

    oneshot: Option<String>,
    oneshot_clip: Option<String>,
    oneshot_drive_on_left: bool,
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
        // Skip the most expensive step of --map, building contraction hierarchies. The resulting
        // map won't be usable for simulation; as soon as you try to pathfind, it'll crash.
        skip_ch: args.enabled("--skip_ch"),

        // Only process one map. If not specified, process all maps defined by clipping polygons in
        // data/input/$city/polygons/.
        only_map: args.optional_free(),

        // Ignore other arguments and just convert the given .osm file to a Map.
        oneshot: args.optional("--oneshot"),
        oneshot_clip: args.optional("--oneshot_clip"),
        oneshot_drive_on_left: args.enabled("--oneshot_drive_on_left"),
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
        oneshot(path, job.oneshot_clip, !job.oneshot_drive_on_left);
        return;
    }

    let names = if let Some(n) = job.only_map {
        println!("- Just working on {}", n);
        vec![n]
    } else {
        println!("- Working on all {} maps", job.city);
        abstutil::list_all_objects(abstutil::path(format!("input/{}/polygons", job.city)))
    };

    let mut timer = abstutil::Timer::new("import map data");

    let (maybe_popdat, maybe_huge_map) = if job.scenario || job.scenario_everyone {
        assert_eq!(job.city, "seattle");

        #[cfg(feature = "scenarios")]
        {
            let (popdat, huge_map) = seattle::ensure_popdat_exists(&mut timer);
            (Some(popdat), Some(huge_map))
        }

        #[cfg(not(feature = "scenarios"))]
        {
            panic!(
                "Can't do --scenario or --scenario_everyone without the scenarios feature \
                 compiled in"
            );
            // Nonsense to make the type-checker work
            (Some(true), Some(true))
        }
    } else {
        (None, None)
    };

    for name in names {
        if job.osm_to_raw {
            match job.city.as_ref() {
                "berlin" => berlin::osm_to_raw(&name),
                "krakow" => krakow::osm_to_raw(&name),
                "seattle" => seattle::osm_to_raw(&name),
                x => panic!("Unknown city {}", x),
            }
        }

        let mut maybe_map = if job.raw_to_map {
            let mut map = utils::raw_to_map(&name, !job.skip_ch, &mut timer);

            // Another strange step in the pipeline.
            if name == "berlin_center" {
                timer.start(format!(
                    "distribute residents from planning areas for {}",
                    name
                ));
                berlin::distribute_residents(&mut map, &mut timer);
                timer.stop(format!(
                    "distribute residents from planning areas for {}",
                    name
                ));
            }

            Some(map)
        } else if job.scenario || job.scenario_everyone {
            Some(map_model::Map::new(abstutil::path_map(&name), &mut timer))
        } else {
            None
        };

        #[cfg(feature = "scenarios")]
        if job.scenario {
            timer.start(format!("scenario for {}", name));
            let scenario = soundcast::make_weekday_scenario(
                maybe_map.as_ref().unwrap(),
                maybe_popdat.as_ref().unwrap(),
                maybe_huge_map.as_ref().unwrap(),
                &mut timer,
            );
            scenario.save();
            timer.stop(format!("scenario for {}", name));

            // This is a strange ordering.
            if name == "downtown" || name == "south_seattle" {
                timer.start(format!("adjust parking for {}", name));
                seattle::adjust_private_parking(maybe_map.as_mut().unwrap(), &scenario);
                timer.stop(format!("adjust parking for {}", name));
            }
        }

        #[cfg(feature = "scenarios")]
        if job.scenario_everyone {
            timer.start(format!("scenario_everyone for {}", name));
            soundcast::make_weekday_scenario_with_everyone(
                maybe_map.as_ref().unwrap(),
                maybe_popdat.as_ref().unwrap(),
                &mut timer,
            )
            .save();
            timer.stop(format!("scenario_everyone for {}", name));
        }
    }
}

fn oneshot(osm_path: String, clip: Option<String>, drive_on_right: bool) {
    let mut timer = abstutil::Timer::new("oneshot");
    println!("- Running convert_osm on {}", osm_path);
    let name = abstutil::basename(&osm_path);
    let raw = convert_osm::convert(
        convert_osm::Options {
            osm_input: osm_path,
            city_name: "oneshot".to_string(),
            name: name.clone(),

            clip,
            map_config: map_model::MapConfig {
                driving_side: if drive_on_right {
                    map_model::raw::DrivingSide::Right
                } else {
                    map_model::raw::DrivingSide::Left
                },
                bikes_can_use_bus_lanes: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            elevation: None,
        },
        &mut timer,
    );
    let map = map_model::Map::create_from_raw(raw, true, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    println!("{} has been created", abstutil::path_map(&name));
}
