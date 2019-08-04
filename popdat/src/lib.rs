pub mod psrc;
mod trips;

use abstutil::Timer;
use geom::{GPSBounds, LonLat};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
pub use trips::{clip_trips, trips_to_scenario, Trip, TripEndpt};

#[derive(Serialize, Deserialize)]
pub struct PopDat {
    // Keyed by census tract label
    // Invariant: Every tract has all data filled out.
    pub tracts: BTreeMap<String, TractData>,

    pub trips: Vec<psrc::Trip>,
    pub parcels: BTreeMap<i64, psrc::Parcel>,
}

#[derive(Serialize, Deserialize)]
pub struct TractData {
    pub pts: Vec<LonLat>,
    pub household_vehicles: BTreeMap<String, Estimate>,
    pub commute_times: BTreeMap<String, Estimate>,
    pub commute_modes: BTreeMap<String, Estimate>,
}

#[derive(Serialize, Deserialize)]
pub struct Estimate {
    pub value: usize,
    // margin of error, 90% confidence
    pub moe: usize,
}

impl PopDat {
    pub fn import_all(timer: &mut Timer) -> PopDat {
        let mut dat = PopDat {
            tracts: BTreeMap::new(),
            trips: Vec::new(),
            parcels: BTreeMap::new(),
        };
        let fields: Vec<(&str, Box<Fn(&mut TractData, BTreeMap<String, Estimate>)>)> = vec![
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
            for mut shape in kml::load(path, &GPSBounds::seattle_bounds(), timer)
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

                setter(
                    dat.tracts.get_mut(&name).unwrap(),
                    group_attribs(shape.attributes),
                );
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

impl fmt::Display for Estimate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ± {}", self.value, self.moe)
    }
}

fn group_attribs(mut attribs: BTreeMap<String, String>) -> BTreeMap<String, Estimate> {
    // Remove useless stuff
    attribs.remove("Internal feature number.");
    attribs.remove("GEO_ID_TRT");

    let mut estimates = BTreeMap::new();
    let mut moes = BTreeMap::new();
    for (k, v) in attribs {
        // These fields in the household_vehicles dataset aren't interesting.
        if k.contains("person hsehold") {
            continue;
        }

        let value = v
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("Unknown value {}={}", k, v));

        if k.starts_with("E1216 - ") {
            estimates.insert(k["E1216 - ".len()..k.len()].to_string(), value);
        } else if k.starts_with("M121616 - ") {
            moes.insert(k["M121616 - ".len()..k.len()].to_string(), value);
        } else {
            panic!("Unknown key {}={}", k, v);
        }
    }

    // If the length is the same but some keys differ, the lookup in moes below will blow up.
    if estimates.len() != moes.len() {
        panic!("estimates and margins of error have different keys, probably");
    }
    estimates
        .into_iter()
        .map(|(key, e)| {
            (
                key.clone(),
                Estimate {
                    value: e,
                    moe: moes[&key],
                },
            )
        })
        .collect()
}

impl TractData {
    // Nontrivial summary
    pub fn total_owned_cars(&self) -> usize {
        let mut sum = 0;
        for (name, est) in &self.household_vehicles {
            match name.as_str() {
                "1 vehicle avail." => sum += est.value,
                "2 vehicles avail." => sum += 2 * est.value,
                "3 vehicles avail." => sum += 3 * est.value,
                // Many more than 4 seems unrealistic
                "4 or more vehicles avail." => sum += 4 * est.value,
                "No vehicle avail." | "Total:" => {}
                _ => panic!("Unknown household_vehicles key {}", name),
            }
        }
        sum
    }
}
