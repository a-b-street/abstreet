// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use {deserialize_car_to_s_map, serialize_car_to_s_map};

use common::{CarID, SPEED_LIMIT};
use control::ControlMap;
use control::stop_signs::{ControlStopSign, TurnPriority};
use dimensioned::si;
use geom::GeomMap;
use map_model::{IntersectionID, TurnID};
use std::collections::HashMap;

use std;
const WAIT_AT_STOP_SIGN: si::Second<f64> = si::Second {
    value_unsafe: 1.5,
    _marker: std::marker::PhantomData,
};

pub trait IntersectionPolicy {
    // This must only be called when the car is ready to enter the intersection.
    fn can_do_turn(
        &mut self,
        car: CarID,
        turn: TurnID,
        time: si::Second<f64>,
        geom_map: &GeomMap,
        control_map: &ControlMap,
    ) -> bool;

    fn on_enter(&self, car: CarID);
    fn on_exit(&mut self, car: CarID);
}

#[derive(Serialize, Deserialize)]
pub struct StopSign {
    id: IntersectionID,
    #[serde(serialize_with = "serialize_car_to_s_map",
            deserialize_with = "deserialize_car_to_s_map")]
    started_waiting_at: HashMap<CarID, si::Second<f64>>,
    accepted: HashMap<CarID, TurnID>,
    waiting: HashMap<CarID, TurnID>,
}

impl StopSign {
    pub fn new(id: IntersectionID) -> StopSign {
        StopSign {
            id,
            started_waiting_at: HashMap::new(),
            accepted: HashMap::new(),
            waiting: HashMap::new(),
        }
    }

    fn conflicts_with_accepted(&self, turn: TurnID, geom_map: &GeomMap) -> bool {
        let base_t = geom_map.get_t(turn);
        self.accepted
            .values()
            .find(|t| base_t.conflicts_with(geom_map.get_t(**t)))
            .is_some()
    }

    fn conflicts_with_waiting_with_higher_priority(
        &self,
        turn: TurnID,
        geom_map: &GeomMap,
        ss: &ControlStopSign,
    ) -> bool {
        let base_t = geom_map.get_t(turn);
        let base_priority = ss.get_priority(turn);
        self.waiting
            .values()
            .find(|t| {
                base_t.conflicts_with(geom_map.get_t(**t)) && ss.get_priority(**t) > base_priority
            })
            .is_some()
    }
}

impl IntersectionPolicy for StopSign {
    fn can_do_turn(
        &mut self,
        car: CarID,
        turn: TurnID,
        time: si::Second<f64>,
        geom_map: &GeomMap,
        control_map: &ControlMap,
    ) -> bool {
        // TODO assert turn is in this intersection

        if self.accepted.contains_key(&car) {
            return true;
        }

        if !self.started_waiting_at.contains_key(&car) {
            self.started_waiting_at.insert(car, time);
        }

        if self.conflicts_with_accepted(turn, geom_map) {
            self.waiting.insert(car, turn);
            return false;
        }

        let ss = &control_map.stop_signs[&self.id];
        if self.conflicts_with_waiting_with_higher_priority(turn, geom_map, ss) {
            self.waiting.insert(car, turn);
            return false;
        }
        if ss.get_priority(turn) == TurnPriority::Stop
            && time - self.started_waiting_at[&car] < WAIT_AT_STOP_SIGN
        {
            self.waiting.insert(car, turn);
            return false;
        }

        self.accepted.insert(car, turn);
        self.waiting.remove(&car);
        self.started_waiting_at.remove(&car);
        true
    }

    fn on_enter(&self, car: CarID) {
        assert!(self.accepted.contains_key(&car));
    }

    fn on_exit(&mut self, car: CarID) {
        assert!(self.accepted.contains_key(&car));
        self.accepted.remove(&car);
    }
}

#[derive(Serialize, Deserialize)]
pub struct TrafficSignal {
    id: IntersectionID,
    accepted: HashMap<CarID, TurnID>,
}

impl TrafficSignal {
    pub fn new(id: IntersectionID) -> TrafficSignal {
        TrafficSignal {
            id,
            accepted: HashMap::new(),
        }
    }
}

impl IntersectionPolicy for TrafficSignal {
    // TODO determine if cars are staying in the intersection past the cycle time.

    fn can_do_turn(
        &mut self,
        car: CarID,
        turn: TurnID,
        time: si::Second<f64>,
        geom_map: &GeomMap,
        control_map: &ControlMap,
    ) -> bool {
        // TODO assert turn is in this intersection

        if self.accepted.contains_key(&car) {
            return true;
        }

        let signal = &control_map.traffic_signals[&self.id];
        let (cycle, remaining_cycle_time) = signal.current_cycle_and_remaining_time(time);

        if !cycle.contains(turn) {
            return false;
        }
        // How long will it take the car to cross the turn?
        let crossing_time = geom_map.get_t(turn).length() / SPEED_LIMIT;
        // TODO account for TIMESTEP

        if crossing_time < remaining_cycle_time {
            self.accepted.insert(car, turn);
            return true;
        }

        false
    }

    fn on_enter(&self, car: CarID) {
        assert!(self.accepted.contains_key(&car));
    }

    fn on_exit(&mut self, car: CarID) {
        assert!(self.accepted.contains_key(&car));
        self.accepted.remove(&car);
    }
}
