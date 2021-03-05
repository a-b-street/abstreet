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

/// TODO Describe. In particular, how are polygons partly or fully outside the map's boundary
/// handled?
/// TODO Add an options struct to specify AM/PM time distribution, lunch trips, etc.
pub fn disaggregate(
    map: &Map,
    zones: &HashMap<String, Polygon>,
    desire_lines: Vec<DesireLine>,
    rng: &mut XorShiftRng,
    timer: &mut Timer,
) -> Vec<PersonSpec> {
    // First decide which zones are relevant for our map. Find all homes and shops for each zone,
    // and make it easy to repeatedly ask for a good random choice of home/work.
    timer.start("match zones");
    let mut zones = create_zones(map, zones);
    for z in zones.values_mut() {
        represent_homes_proportionally(&mut z.homes, map);
        represent_workplaces_proportionally(&mut z.workplaces, map);
        // Make it easy to grab a random home or workplace.
        z.homes.shuffle(rng);
        z.workplaces.shuffle(rng);
    }
    timer.stop("match zones");

    let mut people = Vec::new();
    timer.start("create people");
    'DESIRE: for desire in desire_lines {
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
            let home = match zones.get_mut(&desire.home_zone).unwrap().homes.pop() {
                Some(b) => b,
                None => {
                    warn!("Ran out of homes in {}", desire.home_zone);
                    continue 'DESIRE;
                }
            };
            let work = match zones.get_mut(&desire.work_zone).unwrap().workplaces.pop() {
                Some(b) => b,
                None => {
                    warn!("Ran out of workplaces in {}", desire.work_zone);
                    continue 'DESIRE;
                }
            };

            // Create their schedule
            people.push(PersonSpec {
                orig_id: None,
                origin: TripEndpoint::Bldg(home),
                trips: vec![
                    IndividTrip::new(
                        Time::START_OF_DAY + Duration::hours(7),
                        TripPurpose::Work,
                        TripEndpoint::Bldg(work),
                        desire.mode,
                    ),
                    IndividTrip::new(
                        Time::START_OF_DAY + Duration::hours(17),
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
    homes: Vec<BuildingID>,
    workplaces: Vec<BuildingID>,
}

fn create_zones(map: &Map, input: &HashMap<String, Polygon>) -> HashMap<String, Zone> {
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
            name.clone(),
            Zone {
                polygon: polygon.clone(),
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
                BuildingType::Residential { .. } => {
                    zone.homes.push(b.id);
                }
                BuildingType::ResidentialCommercial(_, _) => {
                    zone.homes.push(b.id);
                    zone.workplaces.push(b.id);
                }
                BuildingType::Commercial(_) => {
                    zone.workplaces.push(b.id);
                }
                BuildingType::Empty => {}
            }
        }
    }

    zones
}

/// Repeat each residential building based on a guess of how many people live there. That way,
/// we're more likely to allocate more people to larger homes.
///
/// The heuristic for people per building is unfortunately very primitive right now, though.
fn represent_homes_proportionally(input: &mut Vec<BuildingID>, map: &Map) {
    let mut output = Vec::new();
    for b in input.drain(..) {
        let n = match map.get_b(b).bldg_type {
            BuildingType::Residential { num_residents, .. }
            | BuildingType::ResidentialCommercial(num_residents, _) => num_residents,
            _ => unreachable!(),
        };
        output.extend(std::iter::repeat(b).take(n));
    }
    *input = output;
}

/// Repeat each commercial building based on a guess of how many people work there. That way,
/// we're more likely to allocate more employees to larger stores.
fn represent_workplaces_proportionally(input: &mut Vec<BuildingID>, map: &Map) {
    let mut output = Vec::new();
    for b in input.drain(..) {
        // We know how many different stores are located in each building, according to OSM. A big
        // mall might have 10 amenities, while standalone shops just have 1. For now, assume 1
        // worker per store.
        let n = map.get_b(b).amenities.len();
        output.extend(std::iter::repeat(b).take(n));
    }
    *input = output;
}
