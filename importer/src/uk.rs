use std::collections::HashMap;
use std::fs::File;

use anyhow::Result;
use serde::Deserialize;

use abstio::path_shared_input;
use abstutil::Timer;
use geom::{GPSBounds, LonLat, Polygon, Ring};
use map_model::raw::RawMap;
use map_model::Map;
use popdat::od::DesireLine;
use sim::TripMode;

use crate::configuration::ImporterConfiguration;
use crate::utils::download;

pub fn import_collision_data(map: &RawMap, config: &ImporterConfiguration, timer: &mut Timer) {
    download(
        config,
        path_shared_input("Road Safety Data - Accidents 2019.csv"),
        "http://data.dft.gov.uk.s3.amazonaws.com/road-accidents-safety-data/DfTRoadSafety_Accidents_2019.zip");

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

pub fn generate_scenario(
    map: &Map,
    config: &ImporterConfiguration,
    timer: &mut Timer,
) -> Result<()> {
    download(
        config,
        path_shared_input("wu03uk_v3.csv"),
        "https://s3-eu-west-1.amazonaws.com/statistics.digitalresources.jisc.ac.uk/dkan/files/FLOW/wu03uk_v3/wu03uk_v3.csv");
    download(
        config,
        path_shared_input("zones_core.geojson"),
        "https://github.com/cyipt/actdev/releases/download/0.1.13/zones_core.geojson",
    );

    let desire_lines = parse_desire_lines(path_shared_input("wu03uk_v3.csv"))?;
    let zones = parse_zones(
        map.get_gps_bounds(),
        path_shared_input("zones_core.geojson"),
    )?;
    println!("{} zones", zones.len());

    Ok(())
}

fn parse_desire_lines(path: String) -> Result<Vec<DesireLine>> {
    let mut output = Vec::new();
    for rec in csv::Reader::from_reader(File::open(path)?).deserialize() {
        let rec: Record = rec?;
        for (mode, number_commuters) in vec![
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

// An entry in wu03uk_v3.csv. For now, ignores people who work from home, take a taxi, motorcycle,
// are a passenger in a car, or use "another method of travel".
#[derive(Debug, Deserialize)]
struct Record {
    #[serde(rename = "Area of usual residence")]
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

    let bytes = abstio::slurp_file(path)?;
    let raw_string = std::str::from_utf8(&bytes)?;
    let geojson = raw_string.parse::<geojson::GeoJson>()?;

    if let geojson::GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            let zone = feature
                .property("geo_code")
                .and_then(|x| x.as_str())
                .ok_or_else(|| anyhow!("no geo_code"))?
                .to_string();
            if let Some(geom) = feature.geometry {
                if let geojson::Value::MultiPolygon(mut raw_polygons) = geom.value {
                    if raw_polygons.len() != 1 {
                        // We'll just one of them arbitrarily
                        warn!(
                            "Zone {} has a multipolygon with {} members",
                            zone,
                            raw_polygons.len()
                        );
                    }
                    match parse_polygon(raw_polygons.pop().unwrap(), gps_bounds) {
                        Ok(polygon) => {
                            zones.insert(zone, polygon);
                        }
                        Err(err) => {
                            warn!("Zone {} has bad geometry: {}", zone, err);
                        }
                    }
                }
            }
        }
    }

    Ok(zones)
}

fn parse_polygon(input: Vec<Vec<Vec<f64>>>, gps_bounds: &GPSBounds) -> Result<Polygon> {
    let mut rings = Vec::new();
    for ring in input {
        let gps_pts: Vec<LonLat> = ring
            .into_iter()
            .map(|pt| LonLat::new(pt[0], pt[1]))
            .collect();
        let pts = gps_bounds.convert(&gps_pts);
        rings.push(Ring::new(pts)?);
    }
    Ok(Polygon::from_rings(rings))
}
