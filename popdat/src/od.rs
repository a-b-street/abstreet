//! This is an alternative pipeline for generating a Scenario, starting from origin-destination
//! data (also called desire lines), which gives a count of commuters between two zones, breaking
//! down by mode.
//!
//! Maybe someday, we'll merge the two approaches, and make the first generate DesireLines as an
//! intermediate step.

use std::collections::HashMap;

use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::Polygon;
use map_model::{BuildingID, BuildingType, Map};
use sim::{PersonSpec, TripMode};

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
    timer.start("match zones");
    let zones = create_zones(map, zones);
    timer.stop("match zones");
    Vec::new()
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
