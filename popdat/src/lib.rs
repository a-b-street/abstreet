use abstutil::Timer;
use geom::{GPSBounds, LonLat};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
pub struct PopDat {
    // Keyed by census tract
    pub household_vehicles: BTreeMap<String, TractData>,
    pub commute_times: BTreeMap<String, TractData>,
    pub commute_modes: BTreeMap<String, TractData>,
}

#[derive(Serialize, Deserialize)]
pub struct TractData {
    pub pts: Vec<LonLat>,
    // TODO measurement and error grouped together, at least, ±
    pub raw: BTreeMap<String, String>,
}

impl PopDat {
    pub fn import_all(timer: &mut Timer) -> PopDat {
        // Generally large slice of Seattle
        let mut bounds = GPSBounds::new();
        bounds.update(LonLat::new(-122.4416, 47.5793));
        bounds.update(LonLat::new(-122.2421, 47.7155));

        PopDat {
            household_vehicles: TractData::import(
                "../data/input/household_vehicles.kml",
                &bounds,
                timer,
            ),
            commute_times: TractData::import("../data/input/commute_time.kml", &bounds, timer),
            commute_modes: TractData::import("../data/input/commute_mode.kml", &bounds, timer),
        }
    }
}

impl TractData {
    fn import(path: &str, bounds: &GPSBounds, timer: &mut Timer) -> BTreeMap<String, TractData> {
        let mut map = BTreeMap::new();

        for mut shape in kml::load(path, bounds, timer)
            .expect(&format!("couldn't load {}", path))
            .shapes
        {
            let name = shape.attributes.remove("TRACT_LBL").unwrap();
            // Remove useless stuff
            shape.attributes.remove("Internal feature number.");
            shape.attributes.remove("GEO_ID_TRT");
            map.insert(
                name,
                TractData {
                    pts: shape.points,
                    raw: shape.attributes,
                },
            );
        }

        map
    }
}
