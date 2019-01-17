// This roughly contains code to specify and instantiate a simulation, not the mechanics of running
// it.

mod a_b_test;
mod load;
mod scenario;

pub use self::a_b_test::{ABTest, ABTestResults};
pub use self::load::{load, SimFlags};
pub use self::scenario::{
    BorderSpawnOverTime, OriginDestination, Scenario, SeedParkedCars, SpawnOverTime,
};
