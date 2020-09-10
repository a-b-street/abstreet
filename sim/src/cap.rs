use crate::{CarID, VehicleType};
use geom::{Duration, Time};
use map_model::{LaneID, Map, Path, PathConstraints, PathStep};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// TODO When do we increase the counter for a zone, and when do we enforce the check? If we check
// when the trip starts and increase when the car enters the zone, then lots of cars starting at
// the same time can exceed the cap. If we check AND reserve when the trip starts, then the cap is
// obeyed, but the "reservation" applies immediately, even if the car is far away from the zone.
// This seems more reasonable for now, but leave it as a flag until GLT clarifies.
const RESERVE_WHEN_STARTING_TRIP: bool = true;

// Note this only indexes into the zones we track here, not all of them in the map.
type ZoneIdx = usize;

// This only caps driving trips.
#[derive(Serialize, Deserialize, Clone)]
pub struct CapSimState {
    lane_to_zone: BTreeMap<LaneID, ZoneIdx>,
    zones: Vec<Zone>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Zone {
    cap: usize,
    entered_in_last_hour: BTreeSet<CarID>,
    // TODO Maybe want sliding windows or something else
    hour_started: Time,
}

impl CapSimState {
    pub fn new(map: &Map) -> CapSimState {
        let mut sim = CapSimState {
            lane_to_zone: BTreeMap::new(),
            zones: Vec::new(),
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

    pub fn car_entering_lane(&mut self, now: Time, car: CarID, lane: LaneID) {
        if RESERVE_WHEN_STARTING_TRIP {
            return;
        }
        if car.1 != VehicleType::Car {
            return;
        }
        let zone = if let Some(idx) = self.lane_to_zone.get(&lane) {
            &mut self.zones[*idx]
        } else {
            return;
        };

        if now - zone.hour_started >= Duration::hours(1) {
            zone.hour_started = Time::START_OF_DAY + Duration::hours(now.get_parts().0);
            zone.entered_in_last_hour.clear();
        }
        zone.entered_in_last_hour.insert(car);
    }

    pub fn allow_trip(&mut self, now: Time, car: CarID, path: &Path) -> bool {
        if car.1 != VehicleType::Car || !RESERVE_WHEN_STARTING_TRIP {
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

                    if zone.entered_in_last_hour.len() >= zone.cap {
                        return false;
                    }
                    zone.entered_in_last_hour.insert(car);
                }
            }
        }
        true
    }

    pub fn get_cap_counter(&self, l: LaneID) -> usize {
        if let Some(idx) = self.lane_to_zone.get(&l) {
            self.zones[*idx].entered_in_last_hour.len()
        } else {
            0
        }
    }
}
