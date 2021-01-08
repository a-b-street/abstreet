//! It's assumed that the importer is run with the current directory as the project repository; aka
//! `./data/` and `./importer/config` must exist.

use abstio::MapName;
use abstutil::basename;

use configuration::{load_configuration, ImporterConfiguration};
use dependencies::are_dependencies_callable;

mod berlin;
mod configuration;
mod dependencies;
mod generic;
mod leeds;
mod london;
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
    city_overview: bool,

    skip_ch: bool,
    keep_bldg_tags: bool,

    only_map: Option<String>,

    oneshot: Option<String>,
    oneshot_clip: Option<String>,
    oneshot_drive_on_left: bool,
    oneshot_dont_infer_sidewalks: bool,
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
        // Produce a city overview from all of the individual maps in a city.
        city_overview: args.enabled("--city_overview"),
        // Skip the most expensive step of --map, building contraction hierarchies. The simulation
        // will use a slower method to pathfind.
        skip_ch: args.enabled("--skip_ch"),
        // Preserve OSM tags for buildings, increasing the file size.
        keep_bldg_tags: args.enabled("--keep_bldg_tags"),

        // Only process one map. If not specified, process all maps defined by clipping polygons in
        // importer/config/$city/.
        only_map: args.optional_free(),

        // Ignore other arguments and just convert the given .osm file to a Map.
        oneshot: args.optional("--oneshot"),
        oneshot_clip: args.optional("--oneshot_clip"),
        oneshot_drive_on_left: args.enabled("--oneshot_drive_on_left"),
        oneshot_dont_infer_sidewalks: args.enabled("--oneshot_dont_infer_sidewalks"),
    };
    args.done();
    if !job.osm_to_raw
        && !job.raw_to_map
        && !job.scenario
        && !job.city_overview
        && job.oneshot.is_none()
    {
        println!(
            "Nothing to do! Pass some combination of --raw, --map, --scenario, --city_overview, \
             or --oneshot"
        );
        std::process::exit(1);
    }

    let config: ImporterConfiguration = load_configuration();

    if job.osm_to_raw {
        if !are_dependencies_callable(&config) {
            println!(
                "One or more dependencies aren't callable. Add them to the path and try again."
            );
            std::process::exit(1);
        }
    }

    if let Some(path) = job.oneshot {
        oneshot(
            path,
            job.oneshot_clip,
            !job.oneshot_drive_on_left,
            !job.oneshot_dont_infer_sidewalks,
            !job.skip_ch,
            job.keep_bldg_tags,
        );
        return;
    }

    let names = if let Some(n) = job.only_map {
        println!("- Just working on {}", n);
        vec![n]
    } else {
        println!("- Working on all {} maps", job.city);
        abstio::list_dir(format!("importer/config/{}", job.city))
            .into_iter()
            .filter(|path| path.ends_with(".poly"))
            .map(basename)
            .collect()
    };

    let mut timer = abstutil::Timer::new("import map data");

    let (maybe_popdat, maybe_huge_map) = if job.scenario {
        assert_eq!(job.city, "seattle");

        #[cfg(feature = "scenarios")]
        {
            let (popdat, huge_map) = seattle::ensure_popdat_exists(&mut timer, &config);
            (Some(popdat), Some(huge_map))
        }

        #[cfg(not(feature = "scenarios"))]
        {
            panic!("Can't do --scenario without the scenarios feature compiled in");
            // Nonsense to make the type-checker work
            (Some(true), Some(true))
        }
    } else {
        (None, None)
    };

    for name in names {
        if job.osm_to_raw {
            // Still special-cased
            if job.city == "seattle" {
                seattle::osm_to_raw(&name, &mut timer, &config);
            } else {
                let raw = match abstio::maybe_read_json::<generic::GenericCityImporter>(
                    format!("importer/config/{}/cfg.json", job.city),
                    &mut timer,
                ) {
                    Ok(city_cfg) => {
                        city_cfg.osm_to_raw(MapName::new(&job.city, &name), &mut timer, &config)
                    }
                    Err(err) => {
                        panic!("Can't import city {}: {}", job.city, err);
                    }
                };

                match job.city.as_ref() {
                    "berlin" => berlin::import_extra_data(&raw, &config, &mut timer),
                    "leeds" => {
                        if name == "huge" {
                            leeds::import_extra_data(&raw, &config, &mut timer);
                        }
                    }
                    "london" => london::import_extra_data(&raw, &config, &mut timer),
                    _ => {}
                }
            }
        }
        let name = MapName::new(&job.city, &name);

        let mut maybe_map = if job.raw_to_map {
            let mut map = utils::raw_to_map(&name, !job.skip_ch, job.keep_bldg_tags, &mut timer);

            // Another strange step in the pipeline.
            if name == MapName::new("berlin", "center") {
                timer.start(format!(
                    "distribute residents from planning areas for {}",
                    name.describe()
                ));
                berlin::distribute_residents(&mut map, &mut timer);
                timer.stop(format!(
                    "distribute residents from planning areas for {}",
                    name.describe()
                ));
            } else if name.city == "seattle" {
                timer.start(format!("add GTFS schedules for {}", name.describe()));
                seattle::add_gtfs_schedules(&mut map);
                timer.stop(format!("add GTFS schedules for {}", name.describe()));
            }

            Some(map)
        } else if job.scenario {
            Some(map_model::Map::new(name.path(), &mut timer))
        } else {
            None
        };

        #[cfg(feature = "scenarios")]
        if job.scenario {
            timer.start(format!("scenario for {}", name.describe()));
            let scenario = soundcast::make_weekday_scenario(
                maybe_map.as_ref().unwrap(),
                maybe_popdat.as_ref().unwrap(),
                maybe_huge_map.as_ref().unwrap(),
                &mut timer,
            );
            scenario.save();
            timer.stop(format!("scenario for {}", name.describe()));

            // This is a strange ordering.
            if name.map == "downtown" || name.map == "south_seattle" {
                timer.start(format!("adjust parking for {}", name.describe()));
                seattle::adjust_private_parking(maybe_map.as_mut().unwrap(), &scenario);
                timer.stop(format!("adjust parking for {}", name.describe()));
            }

            timer.start("match parcels to buildings");
            seattle::match_parcels_to_buildings(maybe_map.as_mut().unwrap(), &mut timer);
            timer.stop("match parcels to buildings");
        }
    }

    if job.city_overview {
        timer.start(format!("generate city overview for {}", job.city));
        abstio::write_binary(
            abstio::path(format!("system/{}/city.bin", job.city)),
            &map_model::City::from_individual_maps(&job.city, &mut timer),
        );
        timer.stop(format!("generate city overview for {}", job.city));
    }
}

fn oneshot(
    osm_path: String,
    clip: Option<String>,
    drive_on_right: bool,
    inferred_sidewalks: bool,
    build_ch: bool,
    keep_bldg_tags: bool,
) {
    let mut timer = abstutil::Timer::new("oneshot");
    println!("- Running convert_osm on {}", osm_path);
    let name = abstutil::basename(&osm_path);
    let raw = convert_osm::convert(
        convert_osm::Options {
            osm_input: osm_path,
            name: MapName::new("oneshot", &name),

            clip,
            map_config: map_model::MapConfig {
                driving_side: if drive_on_right {
                    map_model::DrivingSide::Right
                } else {
                    map_model::DrivingSide::Left
                },
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks,
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            elevation: None,
            include_railroads: true,
        },
        &mut timer,
    );
    // Often helpful to save intermediate representation in case user wants to load into map_editor
    raw.save();
    let map = map_model::Map::create_from_raw(raw, build_ch, keep_bldg_tags, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    println!("{} has been created", map.get_name().path());
}
