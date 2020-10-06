use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use geom::{Duration, Time};
use map_model::{LaneID, Map, Path, PathConstraints, PathRequest, PathStep};

use crate::{CarID, VehicleType};

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

    fn allow_trip(&mut self, now: Time, car: CarID, path: &Path) -> bool {
        if car.1 != VehicleType::Car {
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

    pub fn validate_path(
        &mut self,
        req: &PathRequest,
        path: Path,
        now: Time,
        car: CarID,
        capped: &mut bool,
        map: &Map,
    ) -> Option<Path> {
        if self.allow_trip(now, car, &path) {
            return Some(path);
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
        map.pathfind_avoiding_zones(req.clone(), avoid_lanes)
    }

    pub fn get_cap_counter(&self, l: LaneID) -> usize {
        if let Some(idx) = self.lane_to_zone.get(&l) {
            self.zones[*idx].entered_in_last_hour.len()
        } else {
            0
        }
    }
}
