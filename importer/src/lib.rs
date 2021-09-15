//! It's assumed that the importer is run with the current directory as the project repository; aka
//! `./data/` and `./importer/config` must exist.

// Disable some noisy clippy lints
#![allow(clippy::type_complexity)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use abstio::{CityName, MapName};
use abstutil::Timer;
use geom::Distance;
use map_model::RawToMapOptions;

use configuration::{load_configuration, ImporterConfiguration};

mod berlin;
mod configuration;
mod generic;
mod seattle;
mod soundcast;
mod uk;
mod utils;

// TODO Might be cleaner to express as a dependency graph?

pub async fn run(raw_args: Vec<String>) {
    let config: ImporterConfiguration = load_configuration();

    let mut args = abstutil::CmdArgs::from_args(raw_args);
    let opts = RawToMapOptions {
        build_ch: !args.enabled("--skip_ch"),
        consolidate_all_intersections: args.enabled("--consolidate_all_intersections"),
        keep_bldg_tags: args.enabled("--keep_bldg_tags"),
    };

    if let Some(path) = args.optional("--oneshot") {
        let clip = args.optional("--oneshot_clip");
        let drive_on_left = args.enabled("--oneshot_drive_on_left");
        args.done();

        oneshot(path, clip, !drive_on_left, opts);
        return;
    }

    if args.enabled("--regen_all") {
        assert!(opts.build_ch);
        assert!(!opts.keep_bldg_tags);
        let shard_num = args
            .optional_parse("--shard_num", |s| s.parse::<usize>())
            .unwrap_or(0);
        let num_shards = args
            .optional_parse("--num_shards", |s| s.parse::<usize>())
            .unwrap_or(1);
        regenerate_everything(config, shard_num, num_shards).await;
        return;
    }
    if args.enabled("--regen_all_maps_parallel") {
        assert!(opts.build_ch);
        assert!(!opts.keep_bldg_tags);
        regenerate_all_maps(opts);
        return;
    }

    // Otherwise, we're just operating on a single city.
    let job = Job {
        city: match args.optional("--city") {
            Some(x) => CityName::parse(&x).unwrap(),
            None => CityName::seattle(),
        },
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

    job.run(&config, opts, &mut timer).await;
}

async fn regenerate_everything(config: ImporterConfiguration, shard_num: usize, num_shards: usize) {
    // Discover all cities by looking at config. But always operate on Seattle first. Special
    // treatment ;)
    let mut all_cities = CityName::list_all_cities_from_importer_config();
    all_cities.retain(|x| x != &CityName::seattle());
    all_cities.insert(0, CityName::seattle());

    let mut timer = Timer::new("regenerate all maps");
    for (cnt, city) in all_cities.into_iter().enumerate() {
        let mut job = Job {
            city: city.clone(),
            osm_to_raw: true,
            raw_to_map: true,
            scenario: false,
            city_overview: false,
            only_map: None,
        };
        // Only some maps run extra tasks
        if city == CityName::seattle() || city.country == "gb" {
            job.scenario = true;
        }
        // TODO Autodetect this based on number of maps per city?
        if city == CityName::new("gb", "leeds")
            || city == CityName::new("us", "nyc")
            || city == CityName::new("fr", "charleville_mezieres")
            || city == CityName::new("fr", "paris")
            || city == CityName::new("at", "salzburg")
            || city == CityName::new("ir", "tehran")
        {
            job.city_overview = true;
        }

        if cnt % num_shards == shard_num {
            job.run(&config, RawToMapOptions::default(), &mut timer)
                .await;
        }
    }
}

fn regenerate_all_maps(opts: RawToMapOptions) {
    // Omit Seattle and Berlin, because they have special follow-up actions (GTFS and
    // distributing residents)
    let all_maps: Vec<MapName> = CityName::list_all_cities_from_importer_config()
        .into_iter()
        .flat_map(|city| city.list_all_maps_in_city_from_importer_config())
        .filter(|name| {
            name != &MapName::new("de", "berlin", "center") && name.city != CityName::seattle()
        })
        .collect();
    Timer::new("regenerate all maps").parallelize("import each city", all_maps, |name| {
        // Don't pass in a timer; the logs are way too spammy.
        // It's also recommended to run with RUST_LOG=none
        utils::raw_to_map(&name, opts.clone(), &mut Timer::throwaway())
    });
}

struct Job {
    city: CityName,
    osm_to_raw: bool,
    raw_to_map: bool,
    scenario: bool,
    city_overview: bool,

    only_map: Option<String>,
}

impl Job {
    async fn run(
        self,
        config: &ImporterConfiguration,
        opts: RawToMapOptions,
        timer: &mut Timer<'_>,
    ) {
        timer.start(format!("import {}", self.city.describe()));
        let names = if let Some(n) = self.only_map {
            println!("- Just working on {}", n);
            vec![MapName::from_city(&self.city, &n)]
        } else {
            println!("- Working on all {} maps", self.city.describe());
            self.city.list_all_maps_in_city_from_importer_config()
        };

        // When regenerating everything, huge_seattle gets created twice! This is expensive enough
        // to hack in a way to avoid the work.
        let mut built_raw_huge_seattle = false;
        let mut built_map_huge_seattle = false;
        let (maybe_popdat, maybe_huge_map, maybe_zoning_parcels) = if self.scenario
            && self.city == CityName::seattle()
        {
            timer.start("ensure_popdat_exists");
            let (popdat, huge_map) = seattle::ensure_popdat_exists(
                timer,
                config,
                &mut built_raw_huge_seattle,
                &mut built_map_huge_seattle,
            )
            .await;
            // Just assume --raw has been called...
            let shapes: kml::ExtraShapes =
                abstio::read_binary(CityName::seattle().input_path("zoning_parcels.bin"), timer);
            timer.stop("ensure_popdat_exists");
            (Some(popdat), Some(huge_map), Some(shapes))
        } else {
            (None, None, None)
        };

        for name in names {
            timer.start(name.describe());
            if self.osm_to_raw {
                // Still special-cased
                if name.city == CityName::seattle() {
                    if !built_raw_huge_seattle || name.map != "huge_seattle" {
                        seattle::osm_to_raw(&name.map, timer, config).await;
                    }
                } else {
                    let raw = match abstio::maybe_read_json::<generic::GenericCityImporter>(
                        format!(
                            "importer/config/{}/{}/cfg.json",
                            self.city.country, self.city.city
                        ),
                        timer,
                    ) {
                        Ok(city_cfg) => city_cfg.osm_to_raw(name.clone(), timer, config).await,
                        Err(err) => {
                            panic!("Can't import {}: {}", name.describe(), err);
                        }
                    };

                    if name.city == CityName::new("de", "berlin") {
                        berlin::import_extra_data(&raw, config, timer).await;
                    } else if name == MapName::new("gb", "leeds", "huge") {
                        uk::import_collision_data(&raw, config, timer).await;
                    } else if name.city == CityName::new("gb", "london") {
                        uk::import_collision_data(&raw, config, timer).await;
                    }
                }
            }

            let mut maybe_map = if self.raw_to_map {
                let mut map = if built_map_huge_seattle && name == MapName::seattle("huge_seattle")
                {
                    map_model::Map::load_synchronously(name.path(), timer)
                } else {
                    utils::raw_to_map(&name, opts.clone(), timer)
                };

                // Another strange step in the pipeline.
                if name == MapName::new("de", "berlin", "center") {
                    timer.start(format!(
                        "distribute residents from planning areas for {}",
                        name.describe()
                    ));
                    berlin::distribute_residents(&mut map, timer);
                    timer.stop(format!(
                        "distribute residents from planning areas for {}",
                        name.describe()
                    ));
                } else if name.city == CityName::seattle() {
                    // TODO Slightly misleading, but hijack --skip_ch to also skip GTFS. The
                    // intention of --skip_ch is usually to quickly iterate on the map importer,
                    // not in release mode. This import is broken/unused right now anyway and takes
                    // way too much time in debug mode.
                    if opts.build_ch {
                        timer.start(format!("add GTFS schedules for {}", name.describe()));
                        seattle::add_gtfs_schedules(&mut map);
                        timer.stop(format!("add GTFS schedules for {}", name.describe()));
                    }
                }

                Some(map)
            } else if self.scenario {
                Some(map_model::Map::load_synchronously(name.path(), timer))
            } else {
                None
            };

            if self.scenario {
                if self.city == CityName::seattle() {
                    timer.start(format!("scenario for {}", name.describe()));
                    let scenario = soundcast::make_scenario(
                        "weekday",
                        maybe_map.as_ref().unwrap(),
                        maybe_popdat.as_ref().unwrap(),
                        maybe_huge_map.as_ref().unwrap(),
                        timer,
                    );
                    scenario.save();
                    timer.stop(format!("scenario for {}", name.describe()));

                    if name.map.starts_with("aurora_") {
                        timer.start(format!("pass-through scenario for {}", name.describe()));
                        let secondary_scenario = soundcast::make_scenario(
                            "passthrough",
                            maybe_map.as_ref().unwrap(),
                            maybe_popdat.as_ref().unwrap(),
                            maybe_huge_map.as_ref().unwrap(),
                            timer,
                        );
                        secondary_scenario.save();
                        timer.stop(format!("pass-through scenario for {}", name.describe()));
                    }

                    // This is a strange ordering.
                    if name.map == "downtown"
                        || name.map == "qa"
                        || name.map == "south_seattle"
                        || name.map == "wallingford"
                    {
                        timer.start(format!("adjust parking for {}", name.describe()));
                        seattle::adjust_private_parking(maybe_map.as_mut().unwrap(), &scenario);
                        timer.stop(format!("adjust parking for {}", name.describe()));
                    }

                    timer.start("match parcels to buildings");
                    seattle::match_parcels_to_buildings(
                        maybe_map.as_mut().unwrap(),
                        maybe_zoning_parcels.as_ref().unwrap(),
                        timer,
                    );
                    timer.stop("match parcels to buildings");
                }

                if self.city.country == "gb" {
                    uk::generate_scenario(maybe_map.as_ref().unwrap(), config, timer)
                        .await
                        .unwrap();
                }
            }
            timer.stop(name.describe());
        }

        if self.city_overview {
            timer.start(format!(
                "generate city overview for {}",
                self.city.describe()
            ));
            abstio::write_binary(
                abstio::path(format!(
                    "system/{}/{}/city.bin",
                    self.city.country, self.city.city
                )),
                &map_model::City::from_individual_maps(&self.city, timer),
            );
            timer.stop(format!(
                "generate city overview for {}",
                self.city.describe()
            ));
        }

        timer.stop(format!("import {}", self.city.describe()));
    }
}

fn oneshot(osm_path: String, clip: Option<String>, drive_on_right: bool, opts: RawToMapOptions) {
    let mut timer = abstutil::Timer::new("oneshot");
    println!("- Running convert_osm on {}", osm_path);
    let name = abstutil::basename(&osm_path);
    let raw = convert_osm::convert(
        convert_osm::Options {
            osm_input: osm_path,
            name: MapName::new("zz", "oneshot", &name),

            clip,
            map_config: map_model::MapConfig {
                driving_side: if drive_on_right {
                    map_model::DrivingSide::Right
                } else {
                    map_model::DrivingSide::Left
                },
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
                street_parking_spot_length: Distance::meters(8.0),
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(1),
            include_railroads: true,
            extra_buildings: None,
            skip_local_roads: false,
        },
        &mut timer,
    );
    // Often helpful to save intermediate representation in case user wants to load into map_editor
    raw.save();
    let map = map_model::Map::create_from_raw(raw, opts, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    println!("{} has been created", map.get_name().path());
}
