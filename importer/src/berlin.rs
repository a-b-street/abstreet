use std::fs::File;

use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use serde::Deserialize;

use abstutil::{prettyprint_usize, Timer};
use geom::{Polygon, Ring};
use kml::ExtraShapes;
use map_model::raw::RawMap;
use map_model::BuildingType;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, download_kml};

pub fn import_extra_data(map: &RawMap, config: &ImporterConfiguration, timer: &mut Timer) {
    // From https://data.technologiestiftung-berlin.de/dataset/lor_planungsgraeume/en
    download_kml(
        "input/berlin/planning_areas.bin",
        "https://tsb-opendata.s3.eu-central-1.amazonaws.com/lor_planungsgraeume/lor_planungsraeume.kml",
        &map.gps_bounds,
        // Keep partly out-of-bounds polygons
        false,
        timer
    );

    // From
    // https://daten.berlin.de/datensaetze/einwohnerinnen-und-einwohner-berlin-lor-planungsr%C3%A4umen-am-31122018
    download(
        config,
        "input/berlin/EWR201812E_Matrix.csv",
        "https://www.statistik-berlin-brandenburg.de/opendata/EWR201812E_Matrix.csv",
    );

    // Always do this, it's idempotent and fast
    correlate_population(
        "data/input/berlin/planning_areas.bin",
        "data/input/berlin/EWR201812E_Matrix.csv",
        timer,
    );
}

// Modify the filtered KML of planning areas with the number of residents from a different dataset.
fn correlate_population(kml_path: &str, csv_path: &str, timer: &mut Timer) {
    let mut shapes = abstutil::read_binary::<ExtraShapes>(kml_path.to_string(), timer);
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
    abstutil::write_binary(kml_path.to_string(), &shapes);
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
    for shape in abstutil::read_binary::<ExtraShapes>(
        "data/input/berlin/planning_areas.bin".to_string(),
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
        let region = Ring::must_new(pts).to_polygon();
        let bldgs: Vec<map_model::BuildingID> = map
            .all_buildings()
            .into_iter()
            .filter(|b| region.contains_pt(b.label_center) && b.bldg_type.has_residents())
            .map(|b| b.id)
            .collect();
        let orig_num_residents = shape.attributes["num_residents"].parse::<f64>().unwrap();

        // If the region is partly out-of-bounds, then scale down the number of residents linearly
        // based on area of the overlapping part of the polygon.
        let pct_overlap = Polygon::union_all(region.intersection(map.get_boundary_polygon()))
            .area()
            / region.area();
        let num_residents = (pct_overlap * orig_num_residents) as usize;
        timer.note(format!(
            "Distributing {} residents in {} to {} buildings. {}% of this area overlapped with \
             the map, scaled residents accordingly.",
            prettyprint_usize(num_residents),
            shape.attributes["spatial_alias"],
            prettyprint_usize(bldgs.len()),
            (pct_overlap * 100.0) as usize
        ));

        // Deterministically seed using the planning area's ID.
        let mut rng =
            XorShiftRng::seed_from_u64(shape.attributes["spatial_name"].parse::<u64>().unwrap());

        // How do you randomly distribute num_residents into some buildings?
        // https://stackoverflow.com/questions/2640053/getting-n-random-numbers-whose-sum-is-m
        // TODO Problems:
        // - Because of how we round, the sum might not exactly be num_residents
        // - This is not a uniform distribution, per stackoverflow
        // - Larger buildings should get more people

        let mut rand_nums: Vec<f64> = (0..bldgs.len()).map(|_| rng.gen_range(0.0, 1.0)).collect();
        let sum: f64 = rand_nums.iter().sum();
        for b in bldgs {
            let n = (rand_nums.pop().unwrap() / sum * (num_residents as f64)) as usize;
            let bldg_type = match map.get_b(b).bldg_type {
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
            map.hack_override_bldg_type(b, bldg_type);
        }
    }

    map.save();
}
