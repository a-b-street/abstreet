mod activity_model;
mod external;
mod generator;
mod load;
mod modifier;
mod scenario;
mod spawner;

pub use self::external::{ExternalPerson, ExternalTrip};
pub use self::generator::{
    BorderSpawnOverTime, OriginDestination, ScenarioGenerator, SpawnOverTime,
};
pub use self::load::SimFlags;
pub use self::modifier::ScenarioModifier;
pub use self::scenario::{
    IndividTrip, OffMapLocation, PersonSpec, Scenario, SpawnTrip, TripPurpose,
};
pub use self::spawner::{TripSpawner, TripSpec};
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

// Need to explain this trick -- basically keeps consistency between two different simulations when
// each one might make slightly different sequences of calls to the RNG.
pub fn fork_rng(base_rng: &mut XorShiftRng) -> XorShiftRng {
    XorShiftRng::from_seed([base_rng.next_u32() as u8; 16])
}
