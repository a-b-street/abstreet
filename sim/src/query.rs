// Code to inspect the simulation state.

use dimensioned::si;
use geom::Pt2D;
use map_model::LaneID;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use {Sim, Tick, TripID};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct SimStats {
    pub time: Tick,
    pub canonical_pt_per_trip: BTreeMap<TripID, Pt2D>,
}

impl SimStats {
    pub(crate) fn new(time: Tick) -> SimStats {
        SimStats {
            time,
            canonical_pt_per_trip: BTreeMap::new(),
        }
    }
}

pub struct Benchmark {
    last_real_time: Instant,
    last_sim_time: Tick,
}

impl Benchmark {
    pub fn has_real_time_passed(&self, d: Duration) -> bool {
        self.last_real_time.elapsed() >= d
    }
}

impl Sim {
    pub fn start_benchmark(&self) -> Benchmark {
        Benchmark {
            last_real_time: Instant::now(),
            last_sim_time: self.time,
        }
    }

    pub fn measure_speed(&self, b: &mut Benchmark) -> f64 {
        let dt = abstutil::elapsed_seconds(b.last_real_time) * si::S;
        let speed = (self.time - b.last_sim_time).as_time() / dt;
        b.last_real_time = Instant::now();
        b.last_sim_time = self.time;
        speed.value_unsafe
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

impl Sim {
    pub fn summarize(&self, lanes: &HashSet<LaneID>) -> Summary {
        let (cars_parked, open_parking_spots) = self.parking_state.count(lanes);
        let (moving_cars, stuck_cars, buses) = self.driving_state.count(lanes);
        let (moving_peds, stuck_peds) = self.walking_state.count(lanes);

        Summary {
            cars_parked,
            open_parking_spots,
            moving_cars,
            stuck_cars,
            buses,
            moving_peds,
            stuck_peds,
            // Something else has to calculate this
            trips_with_ab_test_divergence: 0,
        }
    }

    // TODO deprecate this, use the new Summary
    pub fn summary(&self) -> String {
        let (waiting_cars, active_cars) = self.driving_state.get_active_and_waiting_count();
        let (waiting_peds, active_peds) = self.walking_state.get_active_and_waiting_count();
        format!(
            "Time: {0}, {1} / {2} active cars waiting, {3} cars parked, {4} / {5} pedestrians waiting",
            self.time,
            waiting_cars,
            active_cars,
            self.parking_state.total_count(),
            waiting_peds, active_peds,
        )
    }
}

// As of a moment in time, not necessarily the end of the simulation
#[derive(Serialize, Deserialize, Debug)]
pub struct ScoreSummary {
    pub pending_walking_trips: usize,
    pub total_walking_trips: usize,
    // TODO this is actually a duration
    pub total_walking_trip_time: Tick,

    pub pending_driving_trips: usize,
    pub total_driving_trips: usize,
    // TODO this is actually a duration
    pub total_driving_trip_time: Tick,

    // If filled out, the sim took this long to complete.
    // TODO This is maybe not a useful thing to measure; the agents moving at the end don't have
    // others around, so things are stranger for them.
    pub completion_time: Option<Tick>,
}

impl Sim {
    pub fn get_score(&self) -> ScoreSummary {
        let mut s = self.trips_state.get_score(self.time);
        if self.is_done() {
            s.completion_time = Some(self.time);
        }
        s
    }
}
