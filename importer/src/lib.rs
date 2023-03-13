//! It's assumed that the importer is run with the current directory as the project repository; aka
//! `./data/` and `./importer/config` must exist.

// Disable some noisy clippy lints
#![allow(clippy::type_complexity)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use structopt::StructOpt;

use abstio::{CityName, MapName};
use abstutil::Timer;
use map_model::RawToMapOptions;

pub use self::configuration::ImporterConfiguration;
pub use self::pick_geofabrik::pick_geofabrik;
pub use utils::osmium;

mod berlin;
mod configuration;
mod map_config;
mod pick_geofabrik;
mod seattle;
mod soundcast;
mod uk;
mod utils;

/// Regenerate all maps and scenarios from scratch.
pub async fn regenerate_everything(shard_num: usize, num_shards: usize) {
    // Discover all cities by looking at config. But always operate on Seattle first. Special
    // treatment ;)
    let mut all_cities = CityName::list_all_cities_from_importer_config();
    all_cities.retain(|x| x != &CityName::seattle());
    all_cities.insert(0, CityName::seattle());

    let mut timer = Timer::new("regenerate all maps");
    for (cnt, city) in all_cities.into_iter().enumerate() {
        if cnt % num_shards == shard_num {
            let job = Job::full_for_city(city);
            job.run(&mut timer).await;
        }
    }
}

/// Transforms a .osm file to a map in one step.
pub async fn oneshot(
    osm_path: String,
    clip: Option<String>,
    options: convert_osm::Options,
    create_uk_travel_demand_model: bool,
    opts: RawToMapOptions,
) {
    let mut timer = abstutil::Timer::new("oneshot");
    println!("- Running convert_osm on {}", osm_path);
    let name = abstutil::basename(&osm_path);
    let raw = convert_osm::convert(
        osm_path,
        MapName::new("zz", "oneshot", &name),
        clip,
        options,
        &mut timer,
    );
    // Often helpful to save intermediate representation in case user wants to load into map_editor
    raw.save();
    let map = map_model::Map::create_from_raw(raw, opts, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");

    if create_uk_travel_demand_model {
        timer.start("generating UK travel demand model");
        uk::generate_scenario(&map, &ImporterConfiguration::load(), &mut timer)
            .await
            .unwrap();
        timer.stop("generating UK travel demand model");
    }

    println!("{} has been created", map.get_name().path());
}

/// A specification for importing all maps in a single city.
#[derive(StructOpt)]
pub struct Job {
    #[structopt(long, parse(try_from_str = CityName::parse), default_value = "us/seattle")]
    pub city: CityName,
    /// Download all raw input files, then convert OSM to the intermediate RawMap.
    #[structopt(long = "--raw")]
    pub osm_to_raw: bool,
    /// Convert the RawMap to the final Map format.
    #[structopt(long = "--map")]
    pub raw_to_map: bool,
    /// Download trip demand data, then produce the typical weekday scenario.
    #[structopt(long)]
    pub scenario: bool,
    /// Produce a city overview from all of the individual maps in a city.
    #[structopt(long)]
    pub city_overview: bool,

    /// Only process one map. If not specified, process all maps defined by clipping polygons in
    /// importer/config/$city/.
    #[structopt()]
    pub only_map: Option<String>,

    #[structopt(flatten)]
    pub opts: RawToMapOptions,
}

impl Job {
    pub fn full_for_city(city: CityName) -> Job {
        let mut job = Job {
            city: city,
            osm_to_raw: true,
            raw_to_map: true,
            scenario: false,
            city_overview: false,
            only_map: None,
            opts: RawToMapOptions::default(),
        };
        // Only some maps run extra tasks
        if job.city == CityName::seattle() || job.city.country == "gb" {
            job.scenario = true;
        }
        // TODO Autodetect this based on number of maps per city?
        if job.city == CityName::new("ch", "zurich")
            || job.city == CityName::new("gb", "leeds")
            || job.city == CityName::new("gb", "london")
            || job.city == CityName::new("us", "nyc")
            || job.city == CityName::new("fr", "charleville_mezieres")
            || job.city == CityName::new("fr", "paris")
            || job.city == CityName::new("at", "salzburg")
            || job.city == CityName::new("ir", "tehran")
            || job.city == CityName::new("pt", "portugal")
        {
            job.city_overview = true;
        }
        job
    }

    /// Return the command-line flags that should produce this job. Incomplete -- doesn't invert
    /// RawToMapOptions
    pub fn flags(&self) -> Vec<String> {
        // TODO Can structopt do the inversion?
        let mut flags = vec![];
        flags.push(format!("--city={}", self.city.to_path()));
        if self.osm_to_raw {
            flags.push("--raw".to_string());
        }
        if self.raw_to_map {
            flags.push("--map".to_string());
        }
        if self.scenario {
            flags.push("--scenario".to_string());
        }
        if self.city_overview {
            flags.push("--city-overview".to_string());
        }
        if let Some(ref name) = self.only_map {
            flags.push(name.clone());
        }
        flags
    }

    pub async fn run(self, timer: &mut Timer<'_>) {
        if !self.osm_to_raw && !self.raw_to_map && !self.scenario && !self.city_overview {
            println!(
                "Nothing to do! Pass some combination of --raw, --map, --scenario, or --city_overview"
            );
            std::process::exit(1);
        }

        let config = ImporterConfiguration::load();

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
                &config,
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
            if self.osm_to_raw
                && (!built_raw_huge_seattle || name != MapName::seattle("huge_seattle"))
            {
                let raw = utils::osm_to_raw(name.clone(), timer, &config).await;

                // The collision data will only cover one part of London, since we don't have a
                // region-wide map there yet
                if name.city == CityName::new("de", "berlin") {
                    berlin::import_extra_data(&raw, &config, timer).await;
                } else if name == MapName::new("gb", "leeds", "huge") {
                    uk::import_collision_data(&raw, &config, timer).await;
                } else if name == MapName::new("gb", "london", "camden") {
                    uk::import_collision_data(&raw, &config, timer).await;
                }
            }

            let mut maybe_map = if self.raw_to_map {
                let mut map = if built_map_huge_seattle && name == MapName::seattle("huge_seattle")
                {
                    map_model::Map::load_synchronously(name.path(), timer)
                } else {
                    utils::raw_to_map(&name, self.opts.clone(), timer)
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

                    map.save();
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

                    // Even stranger hacks! AFTER generating the scenarios, which requires full
                    // pathfinding, for a few maps, "minify" them to cut down file size for the
                    // bike network tool.
                    if name.map == "central_seattle"
                        || name.map == "north_seattle"
                        || name.map == "south_seattle"
                    {
                        let map = maybe_map.as_mut().unwrap();
                        map.minify(timer);
                        map.save();
                    }
                }

                if self.city.country == "gb" {
                    if name == MapName::new("gb", "london", "central") {
                        // No scenario for Central London, which has buildings stripped out
                        let map = maybe_map.as_mut().unwrap();
                        map.minify_buildings(timer);
                        map.save();
                    } else {
                        uk::generate_scenario(maybe_map.as_ref().unwrap(), &config, timer)
                            .await
                            .unwrap();
                    }
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
