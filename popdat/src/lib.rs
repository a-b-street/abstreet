pub mod psrc;
mod trips;

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use trips::{clip_trips, trips_to_scenario, Trip, TripEndpt};

#[derive(Serialize, Deserialize)]
pub struct PopDat {
    pub trips: Vec<psrc::Trip>,
    pub parcels: BTreeMap<i64, psrc::Parcel>,
}
