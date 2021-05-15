use std::collections::HashMap;
use std::fs::File;

use anyhow::Result;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::Deserialize;

use abstio::path_shared_input;
use abstutil::{prettyprint_usize, Timer};
use geom::{GPSBounds, Polygon};
use map_model::raw::RawMap;
use map_model::Map;
use popdat::od::DesireLine;
use sim::{Scenario, TripEndpoint, TripMode};

use crate::configuration::ImporterConfiguration;
use crate::utils::download;

pub async fn import_collision_data(
    map: &RawMap,
    config: &ImporterConfiguration,
    timer: &mut Timer<'_>,
) {
    download(
        config,
        path_shared_input("Road Safety Data - Accidents 2019.csv"),
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip").await;

    // Always do this, it's idempotent and fast
    let shapes = kml::ExtraShapes::load_csv(
        path_shared_input("Road Safety Data - Accidents 2019.csv"),
        &map.gps_bounds,
        timer,
    )
    .unwrap();
    let collisions = collisions::import_stats19(
        shapes,
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");
    abstio::write_binary(
        map.get_city_name().input_path("collisions.bin"),
        &collisions,
    );
}

pub async fn generate_scenario(
    map: &Map,
    config: &ImporterConfiguration,
    timer: &mut Timer<'_>,
) -> Result<()> {
    timer.start("prepare input");
    download(
        config,
        path_shared_input("wu03ew_v2.csv"),
        "https://s3-eu-west-1.amazonaws.com/statistics.digitalresources.jisc.ac.uk/dkan/files/FLOW/wu03ew_v2/wu03ew_v2.csv").await;
    // https://mapit.mysociety.org/area/45350.html (for geocode) E02004277 is an example place to
    // debug where these zones are.
    download(
        config,
        path_shared_input("zones_core.geojson"),
        "https://github.com/cyipt/actdev/releases/download/0.1.13/zones_core.geojson",
    )
    .await;

    let desire_lines = parse_desire_lines(path_shared_input("wu03ew_v2.csv"))?;
    let zones = parse_zones(
        map.get_gps_bounds(),
        path_shared_input("zones_core.geojson"),
    )?;
    timer.stop("prepare input");

    timer.start("disaggregate");
    // Could plumb this in as a flag to the importer, but it's not critical.
    let mut rng = XorShiftRng::seed_from_u64(42);
    let mut scenario = Scenario::empty(map, "background");
    // Include all buses/trains
    scenario.only_seed_buses = None;
    scenario.people = popdat::od::disaggregate(
        map,
        zones,
        desire_lines,
        popdat::od::Options::default(),
        &mut rng,
        timer,
    );
    // Some zones have very few buildings, and people wind up with a home and workplace that're the
    // same!
    scenario = scenario.remove_weird_schedules();
    info!(
        "Generated background traffic scenario with {} people",
        prettyprint_usize(scenario.people.len())
    );
    timer.stop("disaggregate");

    // Does this map belong to the actdev project?
    match load_study_area(map) {
        Ok(study_area) => {
            // Remove people from the scenario we just generated that live in the study area. The
            // data imported using importer/actdev_scenarios.sh already covers them.
            let before = scenario.people.len();
            scenario.people.retain(|p| match p.origin {
                TripEndpoint::Bldg(b) => !study_area.contains_pt(map.get_b(b).polygon.center()),
                _ => true,
            });
            info!(
                "Removed {} people from the background scenario that live in the study area",
                prettyprint_usize(before - scenario.people.len())
            );

            // Create two scenarios, merging the background traffic with the base/active scenarios.
            let mut base: Scenario = abstio::maybe_read_binary::<Scenario>(
                abstio::path_scenario(map.get_name(), "base"),
                timer,
            )?;
            base.people.extend(scenario.people.clone());
            base.scenario_name = "base_with_bg".to_string();
            base.save();

            let mut go_active: Scenario = abstio::maybe_read_binary(
                abstio::path_scenario(map.get_name(), "go_active"),
                timer,
            )?;
            go_active.people.extend(scenario.people);
            go_active.scenario_name = "go_active_with_bg".to_string();
            go_active.save();
        }
        Err(err) => {
            // We're a "normal" city -- just save the background traffic.
            info!("{} has no study area: {}", map.get_name().describe(), err);
            scenario.save();
        }
    }

    Ok(())
}

fn parse_desire_lines(path: String) -> Result<Vec<DesireLine>> {
    let mut output = Vec::new();
    for rec in csv::Reader::from_reader(File::open(path)?).deserialize() {
        let rec: Record = rec?;
        for &(mode, number_commuters) in &[
            (TripMode::Drive, rec.num_drivers),
            (TripMode::Bike, rec.num_bikers),
            (TripMode::Walk, rec.num_pedestrians),
            (
                TripMode::Transit,
                rec.num_transit1 + rec.num_transit2 + rec.num_transit3,
            ),
        ] {
            if number_commuters > 0 {
                output.push(DesireLine {
                    home_zone: rec.home_zone.clone(),
                    work_zone: rec.work_zone.clone(),
                    mode,
                    number_commuters,
                });
            }
        }
    }
    Ok(output)
}

// An entry in wu03ew_v2.csv. For now, ignores people who work from home, take a taxi, motorcycle,
// are a passenger in a car, or use "another method of travel".
#[derive(Debug, Deserialize)]
struct Record {
    #[serde(rename = "Area of residence")]
    home_zone: String,
    #[serde(rename = "Area of workplace")]
    work_zone: String,
    #[serde(rename = "Underground, metro, light rail, tram")]
    num_transit1: usize,
    #[serde(rename = "Train")]
    num_transit2: usize,
    #[serde(rename = "Bus, minibus or coach")]
    num_transit3: usize,
    #[serde(rename = "Driving a car or van")]
    num_drivers: usize,
    #[serde(rename = "Bicycle")]
    num_bikers: usize,
    #[serde(rename = "On foot")]
    num_pedestrians: usize,
}

// Transforms all zones into the map's coordinate space, no matter how far out-of-bounds they are.
fn parse_zones(gps_bounds: &GPSBounds, path: String) -> Result<HashMap<String, Polygon>> {
    let mut zones = HashMap::new();
    let force_convert = true;
    for (polygon, tags) in Polygon::from_geojson_file(path, gps_bounds, force_convert)? {
        zones.insert(tags.get_result("geo_code")?.to_string(), polygon);
    }
    Ok(zones)
}

fn load_study_area(map: &Map) -> Result<Polygon> {
    let force_convert = false;
    let mut list = Polygon::from_geojson_file(
        abstio::path(format!(
            "system/study_areas/{}.geojson",
            map.get_name().city.city.replace("_", "-")
        )),
        map.get_gps_bounds(),
        force_convert,
    )?;
    if list.len() != 1 {
        bail!("study area geojson has {} polygons", list.len());
    }
    Ok(list.pop().unwrap().0)
}
