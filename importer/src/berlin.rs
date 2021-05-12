use std::fs::File;

use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::Deserialize;

use abstutil::Timer;
use geom::Ring;
use kml::ExtraShapes;
use map_model::raw::RawMap;
use map_model::BuildingType;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, download_kml};

pub async fn import_extra_data(
    map: &RawMap,
    config: &ImporterConfiguration,
    timer: &mut Timer<'_>,
) {
    // From https://data.technologiestiftung-berlin.de/dataset/lor_planungsgraeume/en
    download_kml(
        map.get_city_name().input_path("planning_areas.bin"),
        "https://tsb-opendata.s3.eu-central-1.amazonaws.com/lor_planungsgraeume/lor_planungsraeume.kml",
        &map.gps_bounds,
        // Keep partly out-of-bounds polygons
        false,
        timer
    ).await;

    // From
    // https://daten.berlin.de/datensaetze/einwohnerinnen-und-einwohner-berlin-lor-planungsr%C3%A4umen-am-31122018
    download(
        config,
        map.get_city_name().input_path("EWR201812E_Matrix.csv"),
        "https://www.statistik-berlin-brandenburg.de/opendata/EWR201812E_Matrix.csv",
    )
    .await;

    // Always do this, it's idempotent and fast
    correlate_population(
        map.get_city_name().input_path("planning_areas.bin"),
        map.get_city_name().input_path("EWR201812E_Matrix.csv"),
        timer,
    );
}

// Modify the filtered KML of planning areas with the number of residents from a different dataset.
fn correlate_population(kml_path: String, csv_path: String, timer: &mut Timer) {
    let mut shapes = abstio::read_binary::<ExtraShapes>(kml_path.clone(), timer);
    for rec in csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(File::open(csv_path).unwrap())
        .deserialize()
    {
        let rec: Record = rec.unwrap();
        for shape in &mut shapes.shapes {
            if shape.attributes.get("spatial_name") == Some(&rec.raumid) {
                shape
                    .attributes
                    .insert("num_residents".to_string(), rec.e_e);
                break;
            }
        }
    }
    abstio::write_binary(kml_path, &shapes);
}

#[derive(Debug, Deserialize)]
struct Record {
    // Corresponds with spatial_name from planning_areas
    #[serde(rename = "RAUMID")]
    raumid: String,
    // The total residents in that area
    #[serde(rename = "E_E")]
    e_e: String,
}

pub fn distribute_residents(map: &mut map_model::Map, timer: &mut Timer) {
    for shape in abstio::read_binary::<ExtraShapes>(
        "data/input/de/berlin/planning_areas.bin".to_string(),
        timer,
    )
    .shapes
    {
        let pts = map.get_gps_bounds().convert(&shape.points);
        if pts
            .iter()
            .all(|pt| !map.get_boundary_polygon().contains_pt(*pt))
        {
            continue;
        }
        let region = Ring::must_new(pts).into_polygon();
        // Deterministically seed using the planning area's ID.
        let mut rng =
            XorShiftRng::seed_from_u64(shape.attributes["spatial_name"].parse::<u64>().unwrap());

        for (home, n) in popdat::distribute_population_to_homes(
            geo::Polygon::from(region),
            shape.attributes["num_residents"].parse::<usize>().unwrap(),
            map,
            &mut rng,
        ) {
            let bldg_type = match map.get_b(home).bldg_type {
                BuildingType::Residential {
                    num_housing_units, ..
                } => BuildingType::Residential {
                    num_housing_units,
                    num_residents: n,
                },
                BuildingType::ResidentialCommercial(_, worker_cap) => {
                    BuildingType::ResidentialCommercial(n, worker_cap)
                }
                _ => unreachable!(),
            };
            map.hack_override_bldg_type(home, bldg_type);
        }
    }

    map.save();
}
