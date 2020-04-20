use geom::{Distance, Duration, LonLat, Time};
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrigTrip {
    pub from: Endpoint,
    pub to: Endpoint,
    pub depart_at: Time,
    pub mode: Mode,

    // (household, person within household)
    pub person: (usize, usize),
    // (tour, false is to destination and true is back from dst, trip within half-tour)
    pub seq: (usize, bool, usize),
    pub purpose: (Purpose, Purpose),
    pub trip_time: Duration,
    pub trip_dist: Distance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub pos: LonLat,
    pub osm_building: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct Parcel {
    pub num_households: usize,
    pub num_employees: usize,
    pub offstreet_parking_spaces: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Walk,
    Bike,
    Drive,
    Transit,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Purpose {
    Home,
    Work,
    School,
    Escort,
    PersonalBusiness,
    Shopping,
    Meal,
    Social,
    Recreation,
    Medical,
    ParkAndRideTransfer,
}
