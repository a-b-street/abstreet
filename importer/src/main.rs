//! It's assumed that the importer is run with the current directory as the project repository; aka
//! `./data/` and `./importer/config` must exist.

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use abstio::MapName;
use abstutil::{basename, Timer};
use geom::Distance;

use configuration::{load_configuration, ImporterConfiguration};
use dependencies::are_dependencies_callable;

mod actdev;
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

fn main() {
    let config: ImporterConfiguration = load_configuration();
    if !are_dependencies_callable(&config) {
        println!("One or more dependencies aren't callable. Add them to the path and try again.");
        std::process::exit(1);
    }

    let mut args = abstutil::CmdArgs::new();
    // Skip the most expensive step of --map, building contraction hierarchies. The simulation
    // will use a slower method to pathfind.
    let skip_ch = args.enabled("--skip_ch");
    // Preserve OSM tags for buildings, increasing the file size.
    let keep_bldg_tags = args.enabled("--keep_bldg_tags");

    if let Some(path) = args.optional("--oneshot") {
        let clip = args.optional("--oneshot_clip");
        let drive_on_left = args.enabled("--oneshot_drive_on_left");
        args.done();

        oneshot(path, clip, !drive_on_left, !skip_ch, keep_bldg_tags);
        return;
    }

    if args.enabled("--regen_all") {
        assert!(!skip_ch);
        assert!(!keep_bldg_tags);
        regenerate_everything(config);
        return;
    }

    // Otherwise, we're just operating on a single city.
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

        // Only process one map. If not specified, process all maps defined by clipping polygons in
        // importer/config/$city/.
        only_map: args.optional_free(),
    };
    args.done();

    if !job.osm_to_raw && !job.raw_to_map && !job.scenario && !job.city_overview {
        println!(
            "Nothing to do! Pass some combination of --raw, --map, --scenario, --city_overview, \
             or --oneshot"
        );
        std::process::exit(1);
    }

    let mut timer = Timer::new("import map data");

    job.run(&config, skip_ch, keep_bldg_tags, &mut timer);
}

fn regenerate_everything(config: ImporterConfiguration) {
    let mut timer = Timer::new("regenerate all maps");
    for city in vec![
        "seattle",
        "bellevue",
        "berlin",
        "cambridge",
        "cheshire",
        "detroit",
        "krakow",
        "leeds",
        "london",
        "montreal",
        "nyc",
        "paris",
        "providence",
        "salzburg",
        "tel_aviv",
        "warsaw",
    ] {
        let mut job = Job {
            city: city.to_string(),
            osm_to_raw: true,
            raw_to_map: true,
            scenario: false,
            city_overview: false,
            only_map: None,
        };
        // Only some maps run extra tasks
        if city == "seattle" || city == "cambridge" {
            job.scenario = true;
        }
        if city == "nyc" || city == "paris" || city == "salzburg" {
            job.city_overview = true;
        }

        let skip_ch = false;
        let keep_bldg_tags = false;
        job.run(&config, skip_ch, keep_bldg_tags, &mut timer);
    }
}

struct Job {
    city: String,
    osm_to_raw: bool,
    raw_to_map: bool,
    scenario: bool,
    city_overview: bool,

    only_map: Option<String>,
}

impl Job {
    fn run(
        self,
        config: &ImporterConfiguration,
        skip_ch: bool,
        keep_bldg_tags: bool,
        timer: &mut Timer,
    ) {
        timer.start(format!("import {}", self.city));
        let names = if let Some(n) = self.only_map {
            println!("- Just working on {}", n);
            vec![n]
        } else {
            println!("- Working on all {} maps", self.city);
            abstio::list_dir(format!("importer/config/{}", self.city))
                .into_iter()
                .filter(|path| path.ends_with(".poly"))
                .map(basename)
                .collect()
        };

        let (maybe_popdat, maybe_huge_map) = if self.scenario {
            // TODO This is getting messy!
            if self.city == "cambridge" {
                (None, None)
            } else {
                assert_eq!(self.city, "seattle");

                #[cfg(feature = "scenarios")]
                {
                    let (popdat, huge_map) = seattle::ensure_popdat_exists(timer, config);
                    (Some(popdat), Some(huge_map))
                }

                #[cfg(not(feature = "scenarios"))]
                {
                    panic!("Can't do --scenario without the scenarios feature compiled in");
                    // Nonsense to make the type-checker work
                    (Some(true), Some(true))
                }
            }
        } else {
            (None, None)
        };

        for name in names {
            if self.osm_to_raw {
                // Still special-cased
                if self.city == "seattle" {
                    seattle::osm_to_raw(&name, timer, config);
                } else {
                    let raw = match abstio::maybe_read_json::<generic::GenericCityImporter>(
                        format!("importer/config/{}/cfg.json", self.city),
                        timer,
                    ) {
                        Ok(city_cfg) => {
                            city_cfg.osm_to_raw(MapName::new(&self.city, &name), timer, config)
                        }
                        Err(err) => {
                            panic!("Can't import city {}: {}", self.city, err);
                        }
                    };

                    match self.city.as_ref() {
                        "berlin" => berlin::import_extra_data(&raw, config, timer),
                        "leeds" => {
                            if name == "huge" {
                                leeds::import_extra_data(&raw, config, timer);
                            }
                        }
                        "london" => london::import_extra_data(&raw, config, timer),
                        _ => {}
                    }
                }
            }
            let name = MapName::new(&self.city, &name);

            let mut maybe_map = if self.raw_to_map {
                let mut map = utils::raw_to_map(&name, !skip_ch, keep_bldg_tags, timer);

                // Another strange step in the pipeline.
                if name == MapName::new("berlin", "center") {
                    timer.start(format!(
                        "distribute residents from planning areas for {}",
                        name.describe()
                    ));
                    berlin::distribute_residents(&mut map, timer);
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
            } else if self.scenario {
                Some(map_model::Map::new(name.path(), timer))
            } else {
                None
            };

            if self.scenario {
                #[cfg(feature = "scenarios")]
                if self.city == "seattle" {
                    timer.start(format!("scenario for {}", name.describe()));
                    let scenario = soundcast::make_weekday_scenario(
                        maybe_map.as_ref().unwrap(),
                        maybe_popdat.as_ref().unwrap(),
                        maybe_huge_map.as_ref().unwrap(),
                        timer,
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
                    seattle::match_parcels_to_buildings(maybe_map.as_mut().unwrap(), timer);
                    timer.stop("match parcels to buildings");
                }

                if self.city == "cambridge" {
                    actdev::import_scenarios(maybe_map.as_ref().unwrap(), config).unwrap();
                }
            }
        }

        if self.city_overview {
            timer.start(format!("generate city overview for {}", self.city));
            abstio::write_binary(
                abstio::path(format!("system/{}/city.bin", self.city)),
                &map_model::City::from_individual_maps(&self.city, timer),
            );
            timer.stop(format!("generate city overview for {}", self.city));
        }

        timer.stop(format!("import {}", self.city));
    }
}

fn oneshot(
    osm_path: String,
    clip: Option<String>,
    drive_on_right: bool,
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
                inferred_sidewalks: true,
                separate_cycleways: false,
                street_parking_spot_length: Distance::meters(8.0),
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
