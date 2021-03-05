//! This is an alternative pipeline for generating a Scenario, starting from origin-destination
//! data (also called desire lines), which gives a count of commuters between two zones, breaking
//! down by mode.
//!
//! Maybe someday, we'll merge the two approaches, and make the first generate DesireLines as an
//! intermediate step.

use std::collections::HashMap;

use geom::Polygon;
use map_model::Map;
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
) -> Vec<PersonSpec> {
    Vec::new()
}
