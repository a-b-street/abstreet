use crate::utils::{download, download_kml, osmconvert};
use abstutil::Timer;
use kml::ExtraShapes;
use serde::Deserialize;
use std::fs::File;

fn input() {
    download(
        "input/berlin/osm/berlin-latest.osm.pbf",
        "http://download.geofabrik.de/europe/germany/berlin-latest.osm.pbf",
    );

    let bounds = geom::GPSBounds::from(
        geom::LonLat::read_osmosis_polygon(abstutil::path(
            "input/berlin/polygons/berlin_center.poly",
        ))
        .unwrap(),
    );
    // From https://data.technologiestiftung-berlin.de/dataset/lor_planungsgraeume/en
    download_kml(
        "input/berlin/planning_areas.bin",
        "https://tsb-opendata.s3.eu-central-1.amazonaws.com/lor_planungsgraeume/lor_planungsraeume.kml",
        &bounds,
        // Keep partly out-of-bounds polygons
        false,
    );

    // From
    // https://daten.berlin.de/datensaetze/einwohnerinnen-und-einwohner-berlin-lor-planungsr%C3%A4umen-am-31122018
    download(
        "input/berlin/EWR201812E_Matrix.csv",
        "https://www.statistik-berlin-brandenburg.de/opendata/EWR201812E_Matrix.csv",
    );

    // Always do this, it's idempotent and fast
    correlate_population(
        "data/input/berlin/planning_areas.bin",
        "data/input/berlin/EWR201812E_Matrix.csv",
    );
}

pub fn osm_to_raw(name: &str) {
    input();
    osmconvert(
        "input/berlin/osm/berlin-latest.osm.pbf",
        format!("input/berlin/polygons/{}.poly", name),
        format!("input/berlin/osm/{}.osm", name),
    );

    println!("- Running convert_osm");
    let map = convert_osm::convert(
        convert_osm::Options {
            osm_input: abstutil::path(format!("input/berlin/osm/{}.osm", name)),
            city_name: "berlin".to_string(),
            name: name.to_string(),

            clip: Some(abstutil::path(format!(
                "input/berlin/polygons/{}.poly",
                name
            ))),
            map_config: map_model::MapConfig {
                driving_side: map_model::raw::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
            },

            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(3),
            elevation: None,
        },
        &mut abstutil::Timer::throwaway(),
    );
    let output = abstutil::path(format!("input/raw_maps/{}.bin", name));
    println!("- Saving {}", output);
    abstutil::write_binary(output, &map);
}

// Modify the filtered KML of planning areas with the number of residents from a different dataset.
fn correlate_population(kml_path: &str, csv_path: &str) {
    let mut shapes =
        abstutil::read_binary::<ExtraShapes>(kml_path.to_string(), &mut Timer::throwaway());
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
