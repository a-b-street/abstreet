pub mod psrc;
mod trips;

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use trips::trips_to_scenario;

#[derive(Serialize, Deserialize)]
pub struct PopDat {
    pub trips: Vec<psrc::OrigTrip>,
    pub parcels: BTreeMap<i64, psrc::Parcel>,
}
