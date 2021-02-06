//! This module is used for all cities associated with the https://github.com/cyipt/actdev project.

use anyhow::Result;
use geojson::{Feature, GeoJson, Value};
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use geom::{Duration, LonLat, Time};
use map_model::Map;
use sim::{ExternalPerson, ExternalTrip, ExternalTripEndpoint, Scenario, TripMode};

use crate::configuration::ImporterConfiguration;
use crate::utils::download;

pub fn import_scenarios(map: &Map, config: &ImporterConfiguration) -> Result<()> {
    // TODO This hardcodes for one city right now; generalize.
    download(
        config,
        "input/cambridge/desire_lines_disag.geojson",
        "https://raw.githubusercontent.com/cyipt/actdev/main/data-small/great-kneighton/desire_lines_disag.geojson",
    );

    let bytes = abstio::slurp_file(abstio::path("input/cambridge/desire_lines_disag.geojson"))?;
    let raw_string = std::str::from_utf8(&bytes)?;
    let geojson = raw_string.parse::<GeoJson>()?;
    let mut baseline = Vec::new();
    let mut go_dutch = Vec::new();
    if let GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            let (home, work) = match feature.geometry.as_ref().map(|g| &g.value) {
                Some(Value::LineString(pts)) => {
                    if pts.len() != 2 {
                        bail!("Desire line doesn't have 2 points: {:?}", pts);
                    } else {
                        (
                            LonLat::new(pts[0][0], pts[0][1]),
                            LonLat::new(pts[1][0], pts[1][1]),
                        )
                    }
                }
                _ => {
                    bail!("Geometry isn't a line-string: {:?}", feature);
                }
            };
            for (mode, baseline_key, go_dutch_key) in vec![
                (TripMode::Walk, "foot", "walk_commute_godutch"),
                (TripMode::Bike, "bicycle", "bicycle_commute_godutch"),
                (TripMode::Drive, "car_driver", "car_commute_godutch"),
            ] {
                baseline.push(ODSummary {
                    home,
                    work,
                    mode,
                    count: parse_usize(&feature, baseline_key)?,
                });
                go_dutch.push(ODSummary {
                    home,
                    work,
                    mode,
                    count: parse_usize(&feature, go_dutch_key)?,
                });
            }
        }
    }

    generate_scenario("baseline", baseline, map);
    generate_scenario("go_dutch", go_dutch, map);

    Ok(())
}

fn parse_usize(feature: &Feature, key: &str) -> Result<usize> {
    match feature.property(key).and_then(|value| value.as_f64()) {
        Some(count) => Ok(count as usize),
        None => bail!("{} missing or not a number", key),
    }
}

/// Describes some number of people that have the same home, workplace, and preferred mode. When
/// they're created, the only thing that'll differ between them is exact departure time.
struct ODSummary {
    home: LonLat,
    work: LonLat,
    mode: TripMode,
    count: usize,
}

fn generate_scenario(name: &str, input: Vec<ODSummary>, map: &Map) {
    // Arbitrary but fixed seed
    let mut rng = XorShiftRng::seed_from_u64(42);

    let mut people = Vec::new();
    for od in input {
        for _ in 0..od.count {
            let leave_time = rand_time(&mut rng, Duration::hours(7), Duration::hours(9));
            let return_time = rand_time(&mut rng, Duration::hours(17), Duration::hours(19));
            people.push(ExternalPerson {
                origin: ExternalTripEndpoint::Position(od.home),
                trips: vec![
                    ExternalTrip {
                        departure: leave_time,
                        destination: ExternalTripEndpoint::Position(od.work),
                        mode: od.mode,
                    },
                    ExternalTrip {
                        departure: return_time,
                        destination: ExternalTripEndpoint::Position(od.home),
                        mode: od.mode,
                    },
                ],
            });
        }
    }

    let mut scenario = Scenario::empty(map, name);
    // Include all buses/trains
    scenario.only_seed_buses = None;
    scenario.people = ExternalPerson::import(map, people, false).unwrap();
    scenario.save();
}

// TODO Dedupe the many copies of these
fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    assert!(high > low);
    Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}

fn rand_time(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Time {
    Time::START_OF_DAY + rand_duration(rng, low, high)
}
