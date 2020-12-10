use rand_xorshift::XorShiftRng;

use crate::{CensusPerson, Schedule};

impl CensusPerson {
    pub fn generate_schedule(&self, rng: &mut XorShiftRng) -> Schedule {
        // TODO Maybe first classify into a PersonType, then generate a Schedule? I don't know if
        // the intermediate is useful or not
        todo!()
    }
}
