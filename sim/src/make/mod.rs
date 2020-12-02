//! Everything needed to setup a simulation.
//! <https://dabreegster.github.io/abstreet/trafficsim/travel_demand.html> for context.

use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

pub use self::external::{ExternalPerson, ExternalTrip, ExternalTripEndpoint};
pub use self::generator::{BorderSpawnOverTime, ScenarioGenerator, SpawnOverTime};
pub use self::load::SimFlags;
pub use self::modifier::ScenarioModifier;
pub use self::scenario::{IndividTrip, PersonSpec, Scenario, TripPurpose};
pub use self::spawner::TripEndpoint;
pub(crate) use self::spawner::TripSpec;

mod activity_model;
mod external;
mod generator;
mod load;
mod modifier;
mod scenario;
mod spawner;

/// Need to explain this trick -- basically keeps consistency between two different simulations when
/// each one might make slightly different sequences of calls to the RNG.
pub fn fork_rng(base_rng: &mut XorShiftRng) -> XorShiftRng {
    XorShiftRng::seed_from_u64(base_rng.next_u64())
}
