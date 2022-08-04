//! <https://a-b-street.github.io/docs/tech/trafficsim/travel_demand.html> for context.

use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

pub use self::generator::{BorderSpawnOverTime, ScenarioGenerator, SpawnOverTime};

mod activity_model;
mod generator;

/// Need to explain this trick -- basically keeps consistency between two different simulations when
/// each one might make slightly different sequences of calls to the RNG.
pub fn fork_rng(base_rng: &mut XorShiftRng) -> XorShiftRng {
    XorShiftRng::seed_from_u64(base_rng.next_u64())
}
