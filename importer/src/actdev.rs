//! This module is used for all cities associated with the https://github.com/cyipt/actdev project.

use anyhow::Result;
use geojson::{Feature, GeoJson, Value};
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::{Duration, LonLat, Time};
use map_model::Map;
use sim::{ExternalPerson, ExternalTrip, ExternalTripEndpoint, Scenario, TripMode};

use crate::configuration::ImporterConfiguration;
use crate::utils::download;

pub fn import_scenarios(
    map: &Map,
    config: &ImporterConfiguration,
    timer: &mut Timer,
) -> Result<()> {
    // TODO This hardcodes for one city right now; generalize.
    download(
        config,
        "input/cambridge/desire_lines_disag.geojson",
        "https://raw.githubusercontent.com/cyipt/actdev/main/data-small/great-kneighton/desire_lines_disag.geojson",
    );

    let bytes = abstio::slurp_file(abstio::path("input/cambridge/desire_lines_disag.geojson"))?;
    let raw_string = std::str::from_utf8(&bytes)?;
    let geojson = raw_string.parse::<GeoJson>()?;
    let mut results = Vec::new();
    if let GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            // TODO Convert to geo types and then further convert to a PolyLine?
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

            // TODO Can we get ahold of the raw JSON and run it through serde? That'd be way
            // easier.
            results.push(DesireLine {
                home,
                work,

                foot: parse_usize(&feature, "foot")?,
                bicycle: parse_usize(&feature, "bicycle")?,
                car_driver: parse_usize(&feature, "car_driver")?,

                walk_commute_godutch: parse_usize(&feature, "walk_commute_godutch")?,
                bicycle_commute_godutch: parse_usize(&feature, "bicycle_commute_godutch")?,
                car_commute_godutch: parse_usize(&feature, "car_commute_godutch")?,
            });
        }
    }

    desire_lines_to_scenarios(map, results);

    Ok(())
}

// TODO Can we be more succinct here?
fn parse_usize(feature: &Feature, key: &str) -> Result<usize> {
    match feature.property(key).and_then(|value| value.as_f64()) {
        Some(count) => Ok(count as usize),
        None => bail!("{} missing or not a number", key),
    }
}

struct DesireLine {
    home: LonLat,
    work: LonLat,

    foot: usize,
    bicycle: usize,
    car_driver: usize,

    walk_commute_godutch: usize,
    bicycle_commute_godutch: usize,
    car_commute_godutch: usize,
}

fn desire_lines_to_scenarios(map: &Map, input: Vec<DesireLine>) {
    // Arbitrary but fixed seed
    let mut rng = XorShiftRng::seed_from_u64(42);

    let mut baseline_people = Vec::new();
    let mut go_dutch_people = Vec::new();
    for desire in input {
        // TODO The people in the two scenarios aren't related!
        for (mode, baseline_count, go_dutch_count) in vec![
            (TripMode::Walk, desire.foot, desire.walk_commute_godutch),
            (
                TripMode::Bike,
                desire.bicycle,
                desire.bicycle_commute_godutch,
            ),
            (
                TripMode::Drive,
                desire.car_driver,
                desire.car_commute_godutch,
            ),
        ] {
            for (count, output) in vec![
                (baseline_count, &mut baseline_people),
                (go_dutch_count, &mut go_dutch_people),
            ] {
                for _ in 0..count {
                    let leave_time = rand_time(&mut rng, Duration::hours(7), Duration::hours(9));
                    let return_time = rand_time(&mut rng, Duration::hours(17), Duration::hours(19));

                    output.push(ExternalPerson {
                        origin: ExternalTripEndpoint::Position(desire.home),
                        trips: vec![
                            ExternalTrip {
                                departure: leave_time,
                                destination: ExternalTripEndpoint::Position(desire.work),
                                mode,
                            },
                            ExternalTrip {
                                departure: return_time,
                                destination: ExternalTripEndpoint::Position(desire.home),
                                mode,
                            },
                        ],
                    });
                }
            }
        }
    }

    let mut baseline = Scenario::empty(&map, "baseline");
    // Include all buses/trains
    baseline.only_seed_buses = None;
    baseline.people = ExternalPerson::import(map, baseline_people).unwrap();
    baseline.save();

    let mut go_dutch = Scenario::empty(&map, "go_dutch");
    go_dutch.only_seed_buses = None;
    go_dutch.people = ExternalPerson::import(map, go_dutch_people).unwrap();
    go_dutch.save();
}

// TODO Dedupe the many copies of these
fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    assert!(high > low);
    Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}

fn rand_time(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Time {
    Time::START_OF_DAY + rand_duration(rng, low, high)
}
