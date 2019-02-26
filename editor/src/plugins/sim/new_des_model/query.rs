use geom::{Duration, Pt2D};
use serde_derive::{Deserialize, Serialize};
use sim::TripID;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Serialize, Deserialize, PartialEq)]
pub struct SimStats {
    pub time: Duration,
    pub canonical_pt_per_trip: BTreeMap<TripID, Pt2D>,
}

impl SimStats {
    pub(crate) fn new(time: Duration) -> SimStats {
        SimStats {
            time,
            canonical_pt_per_trip: BTreeMap::new(),
        }
    }
}

pub struct Benchmark {
    pub(crate) last_real_time: Instant,
    pub(crate) last_sim_time: Duration,
}

impl Benchmark {
    pub fn has_real_time_passed(&self, d: std::time::Duration) -> bool {
        self.last_real_time.elapsed() >= d
    }
}

// TODO moving vs stuck shouldn't be an instantaneous judgment -- stuck is if there's an agent
// directly in front limiting speed significantly, or if an intersection isn't allowing movement
// yet
pub struct Summary {
    pub cars_parked: usize,
    pub open_parking_spots: usize,
    pub moving_cars: usize,
    pub stuck_cars: usize,
    pub moving_peds: usize,
    pub stuck_peds: usize,
    pub buses: usize,
    // The agent in one or both worlds is in the requested set of lanes.
    pub trips_with_ab_test_divergence: usize,
}

// As of a moment in time, not necessarily the end of the simulation
#[derive(Serialize, Deserialize, Debug)]
pub struct ScoreSummary {
    pub pending_walking_trips: usize,
    pub total_walking_trips: usize,
    pub total_walking_trip_time: Duration,

    pub pending_driving_trips: usize,
    pub total_driving_trips: usize,
    pub total_driving_trip_time: Duration,

    // If filled out, the sim took this long to complete.
    // TODO This is maybe not a useful thing to measure; the agents moving at the end don't have
    // others around, so things are stranger for them.
    pub completion_time: Option<Duration>,
}
