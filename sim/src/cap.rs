use crate::{CarID, VehicleType};
use geom::{Duration, Time};
use map_model::{LaneID, Map, Path, PathConstraints, PathStep};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// Note this only indexes into the zones we track here, not all of them in the map.
type ZoneIdx = usize;

// This only caps driving trips.
#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct CapSimState {
    lane_to_zone: BTreeMap<LaneID, ZoneIdx>,
    zones: Vec<Zone>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
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

    // TODO This is only called when the trip starts. So if a bunch of drivers all spawn at the
    // same time, they'll all pass the check, but wind up exceeding the cap. Should there be some
    // kind of reservation instead?
    pub fn allow_trip(&self, car: CarID, path: &Path) -> bool {
        if car.1 != VehicleType::Car {
            return true;
        }
        for step in path.get_steps() {
            if let PathStep::Lane(l) = step {
                if let Some(idx) = self.lane_to_zone.get(l) {
                    let zone = &self.zones[*idx];
                    if zone.entered_in_last_hour.len() >= zone.cap {
                        return false;
                    }
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
