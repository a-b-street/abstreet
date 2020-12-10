use rand_xorshift::XorShiftRng;

use crate::{CensusPerson, Config, Schedule};

impl CensusPerson {
    pub fn generate_schedule(&self, config: &Config, rng: &mut XorShiftRng) -> Schedule {
        // TODO Maybe first classify into a PersonType, then generate a Schedule? I don't know if
        // the intermediate is useful or not
        todo!()
    }
}
