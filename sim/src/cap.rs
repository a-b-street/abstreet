use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use geom::{Duration, Time};
use map_model::{LaneID, Map, Path, PathConstraints, PathStep, TurnID};

use crate::mechanics::IntersectionSimState;
use crate::{CarID, SimOptions, VehicleType};

// Note this only indexes into the zones we track here, not all of them in the map.
type ZoneIdx = usize;

/// Dynamically limit driving trips that meet different conditions:
///
/// - trips passing through roads with a per-hour cap
/// - trips passing through roads with agents currently experiencing some delay
///
/// Transform the trips by:
///
/// - cancelling them
/// - delaying them
/// - rerouting them
// TODO I'm not sure a single struct is the right way to manage these combinations.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CapSimState {
    lane_to_zone: BTreeMap<LaneID, ZoneIdx>,
    zones: Vec<Zone>,

    cancel_drivers_delay_threshold: Option<Duration>,
    delay_trips_instead_of_cancelling: Option<Duration>,
}

pub enum CapResult {
    OK(Path),
    Reroute(Path),
    Cancel { reason: String },
    Delay(Duration),
    // TODO Switch modes
}

#[derive(Serialize, Deserialize, Clone)]
struct Zone {
    cap: usize,
    entered_in_last_hour: BTreeSet<CarID>,
    // TODO Maybe want sliding windows or something else
    hour_started: Time,
}

impl CapSimState {
    pub fn new(map: &Map, opts: &SimOptions) -> CapSimState {
        let mut sim = CapSimState {
            lane_to_zone: BTreeMap::new(),
            zones: Vec::new(),
            cancel_drivers_delay_threshold: opts.cancel_drivers_delay_threshold.clone(),
            delay_trips_instead_of_cancelling: opts.delay_trips_instead_of_cancelling.clone(),
        };
        for z in map.all_zones() {
            if let Some(cap) = z.restrictions.cap_vehicles_per_hour {
                let idx = sim.zones.len();
                for r in &z.members {
                    for l in map.get_r(*r).all_lanes() {
                        if PathConstraints::Car.can_use(map.get_l(l), map) {
                            sim.lane_to_zone.insert(l, idx);
                        }
                    }
                }
                sim.zones.push(Zone {
                    cap,
                    entered_in_last_hour: BTreeSet::new(),
                    hour_started: Time::START_OF_DAY,
                });
            }
        }
        sim
    }

    /// Before the driving portion of a trip begins, check that the desired path doesn't exceed any
    /// dynamic limits.
    pub fn maybe_cap_path(
        &mut self,
        path: Path,
        now: Time,
        car: CarID,
        intersections: &IntersectionSimState,
        map: &Map,
    ) -> CapResult {
        if self.cancel_drivers_delay_threshold.is_some() {
            if let Some((turn, delay)) = self.path_crosses_delay(now, &path, intersections, map) {
                // TODO Reroute around current delays?
                if let Some(delay) = self.delay_trips_instead_of_cancelling {
                    return CapResult::Delay(delay);
                } else {
                    return CapResult::Cancel {
                        reason: format!("path crosses delay of {} at {}", delay, turn),
                    };
                }
            }
        }

        if self.trip_under_cap(now, car, &path) {
            return CapResult::OK(path);
        }

        let mut avoid_lanes: BTreeSet<LaneID> = BTreeSet::new();
        for (l, idx) in &self.lane_to_zone {
            let zone = &self.zones[*idx];
            if zone.entered_in_last_hour.len() >= zone.cap
                && !zone.entered_in_last_hour.contains(&car)
            {
                avoid_lanes.insert(*l);
            }
        }
        match map.pathfind_avoiding_lanes(path.get_req().clone(), avoid_lanes) {
            Some(path) => CapResult::Reroute(path),
            None => {
                if let Some(delay) = self.delay_trips_instead_of_cancelling {
                    CapResult::Delay(delay)
                } else {
                    CapResult::Cancel {
                        reason: format!("no path avoiding caps: {}", path.get_req()),
                    }
                }
            }
        }
    }
}

// Specific to the cap-per-road mechanism
impl CapSimState {
    pub fn get_cap_counter(&self, l: LaneID) -> usize {
        if let Some(idx) = self.lane_to_zone.get(&l) {
            self.zones[*idx].entered_in_last_hour.len()
        } else {
            0
        }
    }

    fn trip_under_cap(&mut self, now: Time, car: CarID, path: &Path) -> bool {
        if car.1 != VehicleType::Car || self.lane_to_zone.is_empty() {
            return true;
        }
        for step in path.get_steps() {
            if let PathStep::Lane(l) = step {
                if let Some(idx) = self.lane_to_zone.get(l) {
                    let zone = &mut self.zones[*idx];

                    if now - zone.hour_started >= Duration::hours(1) {
                        zone.hour_started = Time::START_OF_DAY + Duration::hours(now.get_parts().0);
                        zone.entered_in_last_hour.clear();
                    }

                    if zone.entered_in_last_hour.len() >= zone.cap
                        && !zone.entered_in_last_hour.contains(&car)
                    {
                        return false;
                    }
                    zone.entered_in_last_hour.insert(car);
                }
            }
        }
        true
    }
}

// Specific to the don't-exceed-delay mechanism
impl CapSimState {
    fn path_crosses_delay(
        &self,
        now: Time,
        path: &Path,
        intersections: &IntersectionSimState,
        map: &Map,
    ) -> Option<(TurnID, Duration)> {
        let threshold = self.cancel_drivers_delay_threshold.unwrap();

        for step in path.get_steps() {
            if let PathStep::Lane(l) = step {
                let lane = map.get_l(*l);
                for (agent, turn, start) in intersections.get_waiting_agents(lane.dst_i) {
                    if now - start < threshold {
                        continue;
                    }
                    if agent.to_vehicle_type() != Some(VehicleType::Car) {
                        continue;
                    }
                    if map.get_l(turn.src).parent != lane.parent {
                        continue;
                    }
                    // TODO Should we make sure the delayed agent is also trying to go the same
                    // direction? For example, people turning left somewhere might be delayed, while
                    // people going straight are fine. But then the presence of a turn lane matters.
                    return Some((turn, now - start));
                }
            }
        }
        None
    }
}
