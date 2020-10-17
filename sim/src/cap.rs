use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use geom::{Duration, Time};
use map_model::{LaneID, Map, Path, PathConstraints, PathRequest, PathStep, TurnID};

use crate::mechanics::IntersectionSimState;
use crate::{CarID, SimOptions, VehicleType};

// Note this only indexes into the zones we track here, not all of them in the map.
type ZoneIdx = usize;

/// Some roads (grouped into zones) may have a cap on the number of vehicles that can enter per
/// hour. CapSimState enforces this, just for driving trips.
#[derive(Serialize, Deserialize, Clone)]
pub struct CapSimState {
    lane_to_zone: BTreeMap<LaneID, ZoneIdx>,
    zones: Vec<Zone>,
    avoid_congestion: Option<AvoidCongestion>,
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
            avoid_congestion: opts
                .cancel_drivers_delay_threshold
                .map(|delay_threshold| AvoidCongestion { delay_threshold }),
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

    fn allow_trip(&mut self, now: Time, car: CarID, path: &Path) -> bool {
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

    /// Before the driving portion of a trip begins, check that the desired path doesn't exceed any
    /// caps. If so, attempt to reroute around.
    pub fn validate_path(
        &mut self,
        req: &PathRequest,
        path: Path,
        now: Time,
        car: CarID,
        capped: &mut bool,
        intersections: &IntersectionSimState,
        map: &Map,
    ) -> Result<Path, String> {
        if let Some(ref avoid) = self.avoid_congestion {
            if let Some((turn, delay)) = avoid.path_crosses_delay(now, &path, intersections, map) {
                *capped = true;
                // TODO More responses
                return Err(format!("path crosses delay of {} at {}", delay, turn));
            }
        }

        if self.allow_trip(now, car, &path) {
            return Ok(path);
        }
        *capped = true;

        // TODO Make the responses configurable: cancel the trip, reroute, delay an hour, switch
        // modes. Where should this policy be specified? Is it simulation-wide?

        let mut avoid_lanes: BTreeSet<LaneID> = BTreeSet::new();
        for (l, idx) in &self.lane_to_zone {
            let zone = &self.zones[*idx];
            if zone.entered_in_last_hour.len() >= zone.cap
                && !zone.entered_in_last_hour.contains(&car)
            {
                avoid_lanes.insert(*l);
            }
        }
        map.pathfind_avoiding_lanes(req.clone(), avoid_lanes)
            .ok_or_else(|| format!("no path avoiding caps: {}", req))
    }

    pub fn get_cap_counter(&self, l: LaneID) -> usize {
        if let Some(idx) = self.lane_to_zone.get(&l) {
            self.zones[*idx].entered_in_last_hour.len()
        } else {
            0
        }
    }
}

/// Before the driving portion of a trip begins, check that the desired path doesn't pass through
/// any road with agents currently experiencing some delay.
#[derive(Serialize, Deserialize, Clone)]
struct AvoidCongestion {
    delay_threshold: Duration,
}

impl AvoidCongestion {
    fn path_crosses_delay(
        &self,
        now: Time,
        path: &Path,
        intersections: &IntersectionSimState,
        map: &Map,
    ) -> Option<(TurnID, Duration)> {
        for step in path.get_steps() {
            if let PathStep::Lane(l) = step {
                let lane = map.get_l(*l);
                for (agent, turn, start) in intersections.get_waiting_agents(lane.dst_i) {
                    if now - start < self.delay_threshold {
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
