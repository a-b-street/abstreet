use abstutil::Timer;
use geom::{GPSBounds, LonLat};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
pub struct PopDat {
    // Keyed by census tract label
    // Invariant: Every tract has all data filled out.
    pub tracts: BTreeMap<String, TractData>,
}

#[derive(Serialize, Deserialize)]
pub struct TractData {
    pub pts: Vec<LonLat>,
    // TODO measurement and error grouped together, at least, ±
    pub household_vehicles: BTreeMap<String, String>,
    pub commute_times: BTreeMap<String, String>,
    pub commute_modes: BTreeMap<String, String>,
}

impl PopDat {
    pub fn import_all(timer: &mut Timer) -> PopDat {
        // Generally large slice of Seattle
        let mut bounds = GPSBounds::new();
        bounds.update(LonLat::new(-122.4416, 47.5793));
        bounds.update(LonLat::new(-122.2421, 47.7155));

        let mut dat = PopDat {
            tracts: BTreeMap::new(),
        };
        let fields: Vec<(&str, Box<Fn(&mut TractData, BTreeMap<String, String>)>)> = vec![
            (
                "../data/input/household_vehicles.kml",
                Box::new(|tract, map| {
                    tract.household_vehicles = map;
                }),
            ),
            (
                "../data/input/commute_time.kml",
                Box::new(|tract, map| {
                    tract.commute_times = map;
                }),
            ),
            (
                "../data/input/commute_mode.kml",
                Box::new(|tract, map| {
                    tract.commute_modes = map;
                }),
            ),
        ];
        for (path, setter) in fields {
            for mut shape in kml::load(path, &bounds, timer)
                .expect(&format!("couldn't load {}", path))
                .shapes
            {
                let name = shape.attributes.remove("TRACT_LBL").unwrap();

                if let Some(ref tract) = dat.tracts.get(&name) {
                    assert_eq!(shape.points, tract.pts);
                } else {
                    dat.tracts.insert(
                        name.clone(),
                        TractData {
                            pts: shape.points,
                            household_vehicles: BTreeMap::new(),
                            commute_times: BTreeMap::new(),
                            commute_modes: BTreeMap::new(),
                        },
                    );
                }

                // Remove useless stuff
                shape.attributes.remove("Internal feature number.");
                shape.attributes.remove("GEO_ID_TRT");

                setter(dat.tracts.get_mut(&name).unwrap(), shape.attributes);
            }
        }

        for (name, tract) in &dat.tracts {
            if tract.household_vehicles.is_empty()
                || tract.commute_times.is_empty()
                || tract.commute_modes.is_empty()
            {
                panic!("{} is missing data", name);
            }
        }

        dat
    }
}
