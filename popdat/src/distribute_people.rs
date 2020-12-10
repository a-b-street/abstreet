use rand_xorshift::XorShiftRng;

use map_model::Map;

use crate::{CensusArea, CensusPerson, Config};

pub fn assign_people_to_houses(
    areas: Vec<CensusArea>,
    map: &Map,
    rng: &mut XorShiftRng,
    config: &Config,
) -> Vec<CensusPerson> {
    // TODO We should generalize the approach of distribute_residents from importer/src/berlin.rs
    todo!()
}
