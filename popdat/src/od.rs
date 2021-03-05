//! This is an alternative pipeline for generating a Scenario, starting from origin-destination
//! data (also called desire lines), which gives a count of commuters between two zones, breaking
//! down by mode.
//!
//! Maybe someday, we'll merge the two approaches, and make the first generate DesireLines as an
//! intermediate step.

use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::{Duration, Polygon, Time};
use map_model::{BuildingID, BuildingType, Map};
use sim::{IndividTrip, PersonSpec, TripEndpoint, TripMode, TripPurpose};

/// This describes some number of commuters living in some named zone, working in another (or the
/// same zone), and commuting using some mode.
#[derive(Debug)]
pub struct DesireLine {
    pub home_zone: String,
    pub work_zone: String,
    pub mode: TripMode,
    pub number_commuters: usize,
}

pub struct Options {
    /// When should somebody depart from home to work?
    pub departure_time: NormalDistribution,
    /// How long should somebody work before returning home?
    pub work_duration: NormalDistribution,
}

impl Options {
    pub fn default() -> Options {
        Options {
            departure_time: NormalDistribution::new(
                Duration::hours(8) + Duration::minutes(30),
                Duration::minutes(30),
            ),
            work_duration: NormalDistribution::new(Duration::hours(9), Duration::hours(1)),
        }
    }
}

/// TODO Describe. In particular, how are polygons partly or fully outside the map's boundary
/// handled?
/// TODO Add an options struct to specify AM/PM time distribution, lunch trips, etc.
pub fn disaggregate(
    map: &Map,
    zones: HashMap<String, Polygon>,
    desire_lines: Vec<DesireLine>,
    opts: Options,
    rng: &mut XorShiftRng,
    timer: &mut Timer,
) -> Vec<PersonSpec> {
    // First decide which zones are relevant for our map. Find all homes and shops for each zone,
    // and make it easy to repeatedly ask for a good random choice of home/work.
    timer.start("match zones");
    let zones = create_zones(map, zones);
    timer.stop("match zones");

    let mut people = Vec::new();
    timer.start("create people");
    for desire in desire_lines {
        // Skip if we filtered out either zone.
        if !zones.contains_key(&desire.home_zone) || !zones.contains_key(&desire.work_zone) {
            continue;
        }

        // Scale the number of commuters by how much the zone overlaps our map.
        // TODO Handle off-map trips better.
        let num_commuters =
            (zones[&desire.home_zone].pct_overlap * (desire.number_commuters as f64)) as usize;
        for _ in 0..num_commuters {
            // Pick a specific home and workplace.
            let home = zones[&desire.home_zone]
                .homes
                .choose_weighted(rng, |(_, n)| *n)
                .unwrap()
                .0;
            let work = zones[&desire.work_zone]
                .workplaces
                .choose_weighted(rng, |(_, n)| *n)
                .unwrap()
                .0;

            // Create their schedule
            let goto_work = Time::START_OF_DAY + opts.departure_time.sample(rng);
            let return_home = goto_work + opts.work_duration.sample(rng);
            people.push(PersonSpec {
                orig_id: None,
                origin: TripEndpoint::Bldg(home),
                trips: vec![
                    IndividTrip::new(
                        goto_work,
                        TripPurpose::Work,
                        TripEndpoint::Bldg(work),
                        desire.mode,
                    ),
                    IndividTrip::new(
                        return_home,
                        TripPurpose::Home,
                        TripEndpoint::Bldg(home),
                        desire.mode,
                    ),
                ],
            });
        }
    }
    timer.stop("create people");
    people
}

struct Zone {
    polygon: Polygon,
    pct_overlap: f64,
    // For each building, have a value describing how many people live or work there. The exact
    // value doesn't matter; it's just a relative weighting. This way, we can use a weighted sample
    // and match more people to larger homes/stores.
    homes: Vec<(BuildingID, usize)>,
    workplaces: Vec<(BuildingID, usize)>,
}

fn create_zones(map: &Map, input: HashMap<String, Polygon>) -> HashMap<String, Zone> {
    let mut zones = HashMap::new();
    for (name, polygon) in input {
        let mut overlapping_area = 0.0;
        for p in polygon.intersection(map.get_boundary_polygon()) {
            overlapping_area += p.area();
        }
        let pct_overlap = overlapping_area / polygon.area();

        // If the zone doesn't intersect our map at all, totally skip it.
        if pct_overlap == 0.0 {
            continue;
        }
        zones.insert(
            name,
            Zone {
                polygon,
                pct_overlap,
                homes: Vec::new(),
                workplaces: Vec::new(),
            },
        );
    }

    // Match all buildings to a zone.
    for b in map.all_buildings() {
        let center = b.polygon.center();
        // We're assuming zones don't overlap each other, so just look for the first match.
        if let Some((_, zone)) = zones
            .iter_mut()
            .find(|(_, z)| z.polygon.contains_pt(center))
        {
            match b.bldg_type {
                BuildingType::Residential { num_residents, .. } => {
                    zone.homes.push((b.id, num_residents));
                }
                BuildingType::ResidentialCommercial(num_residents, _) => {
                    zone.homes.push((b.id, num_residents));
                    // We know how many different stores are located in each building, according to
                    // OSM. A big mall might have 10 amenities, while standalone
                    // shops just have 1.
                    zone.workplaces.push((b.id, b.amenities.len()));
                }
                BuildingType::Commercial(_) => {
                    zone.workplaces.push((b.id, b.amenities.len()));
                }
                BuildingType::Empty => {}
            }
        }
    }

    zones
}

/// A normal distribution of Durations.
pub struct NormalDistribution {
    pub mean: Duration,
    pub std_deviation: Duration,
}

impl NormalDistribution {
    pub fn new(mean: Duration, std_deviation: Duration) -> NormalDistribution {
        NormalDistribution {
            mean,
            std_deviation,
        }
    }

    pub fn sample(&self, rng: &mut XorShiftRng) -> Duration {
        use rand_distr::{Distribution, Normal};

        Duration::seconds(
            Normal::new(
                self.mean.inner_seconds(),
                self.std_deviation.inner_seconds(),
            )
            .unwrap()
            .sample(rng),
        )
    }
}
