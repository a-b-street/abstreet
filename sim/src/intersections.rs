// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use control::stop_signs::{ControlStopSign, TurnPriority};
use control::ControlMap;
use dimensioned::si;
use map_model::{IntersectionID, Map, TurnID};
use std::collections::HashMap;
use {CarID, Tick, SPEED_LIMIT};

use std;
const WAIT_AT_STOP_SIGN: si::Second<f64> = si::Second {
    value_unsafe: 1.5,
    _marker: std::marker::PhantomData,
};

// Use an enum instead of traits so that serialization works. I couldn't figure out erased_serde.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub enum IntersectionPolicy {
    StopSignPolicy(StopSign),
    TrafficSignalPolicy(TrafficSignal),
}

impl IntersectionPolicy {
    // This must only be called when the car is ready to enter the intersection.
    pub fn can_do_turn(
        &mut self,
        car: CarID,
        turn: TurnID,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
    ) -> bool {
        match *self {
            IntersectionPolicy::StopSignPolicy(ref mut p) => {
                p.can_do_turn(car, turn, time, map, control_map)
            }
            IntersectionPolicy::TrafficSignalPolicy(ref mut p) => {
                p.can_do_turn(car, turn, time, map, control_map)
            }
        }
    }

    pub fn on_enter(&self, car: CarID) {
        match self {
            IntersectionPolicy::StopSignPolicy(p) => p.on_enter(car),
            IntersectionPolicy::TrafficSignalPolicy(p) => p.on_enter(car),
        }
    }
    pub fn on_exit(&mut self, car: CarID) {
        match *self {
            IntersectionPolicy::StopSignPolicy(ref mut p) => p.on_exit(car),
            IntersectionPolicy::TrafficSignalPolicy(ref mut p) => p.on_exit(car),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct StopSign {
    id: IntersectionID,
    started_waiting_at: HashMap<CarID, Tick>,
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

    fn conflicts_with_accepted(&self, turn: TurnID, map: &Map) -> bool {
        let base_t = map.get_t(turn);
        self.accepted
            .values()
            .find(|t| base_t.conflicts_with(map.get_t(**t)))
            .is_some()
    }

    fn conflicts_with_waiting_with_higher_priority(
        &self,
        turn: TurnID,
        map: &Map,
        ss: &ControlStopSign,
    ) -> bool {
        let base_t = map.get_t(turn);
        let base_priority = ss.get_priority(turn);
        self.waiting
            .values()
            .find(|t| base_t.conflicts_with(map.get_t(**t)) && ss.get_priority(**t) > base_priority)
            .is_some()
    }

    fn can_do_turn(
        &mut self,
        car: CarID,
        turn: TurnID,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
    ) -> bool {
        // TODO assert turn is in this intersection

        if self.accepted.contains_key(&car) {
            return true;
        }

        if !self.started_waiting_at.contains_key(&car) {
            self.started_waiting_at.insert(car, time);
        }

        if self.conflicts_with_accepted(turn, map) {
            self.waiting.insert(car, turn);
            return false;
        }

        let ss = &control_map.stop_signs[&self.id];
        if self.conflicts_with_waiting_with_higher_priority(turn, map, ss) {
            self.waiting.insert(car, turn);
            return false;
        }
        if ss.get_priority(turn) == TurnPriority::Stop
            && (time - self.started_waiting_at[&car]).as_time() < WAIT_AT_STOP_SIGN
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

#[derive(Serialize, Deserialize, PartialEq, Eq)]
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

    // TODO determine if cars are staying in the intersection past the cycle time.

    fn can_do_turn(
        &mut self,
        car: CarID,
        turn: TurnID,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
    ) -> bool {
        // TODO assert turn is in this intersection

        if self.accepted.contains_key(&car) {
            return true;
        }

        let signal = &control_map.traffic_signals[&self.id];
        let (cycle, remaining_cycle_time) = signal.current_cycle_and_remaining_time(time.as_time());

        if !cycle.contains(turn) {
            return false;
        }
        // How long will it take the car to cross the turn?
        let crossing_time = map.get_t(turn).length() / SPEED_LIMIT;
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
