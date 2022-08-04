//! Everything needed to setup a simulation.

pub use self::load::SimFlags;
pub(crate) use self::spawner::{StartTripArgs, TripSpec};

mod load;
mod spawner;
