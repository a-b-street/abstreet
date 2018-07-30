// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil::{deserialize_btreemap, serialize_btreemap};
use control::stop_signs::{ControlStopSign, TurnPriority};
use control::ControlMap;
use dimensioned::si;
use map_model::{IntersectionID, Map, TurnID};
use std::collections::BTreeMap;
use {CarID, PedestrianID, Tick, SPEED_LIMIT};

use std;
const WAIT_AT_STOP_SIGN: si::Second<f64> = si::Second {
    value_unsafe: 1.5,
    _marker: std::marker::PhantomData,
};

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum AgentID {
    Car(CarID),
    Pedestrian(PedestrianID),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Request {
    pub agent: AgentID,
    pub turn: TurnID,
}

impl Request {
    pub fn for_car(car: CarID, t: TurnID) -> Request {
        Request {
            agent: AgentID::Car(car),
            turn: t,
        }
    }

    pub fn for_ped(ped: PedestrianID, t: TurnID) -> Request {
        Request {
            agent: AgentID::Pedestrian(ped),
            turn: t,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct IntersectionSimState {
    intersections: Vec<IntersectionPolicy>,
}

impl IntersectionSimState {
    pub fn new(map: &Map) -> IntersectionSimState {
        let mut intersections: Vec<IntersectionPolicy> = Vec::new();
        for i in map.all_intersections() {
            if i.has_traffic_signal {
                intersections.push(IntersectionPolicy::TrafficSignalPolicy(TrafficSignal::new(
                    i.id,
                )));
            } else {
                intersections.push(IntersectionPolicy::StopSignPolicy(StopSign::new(i.id)));
            }
        }
        IntersectionSimState { intersections }
    }

    // This must only be called when the agent is ready to enter the intersection.
    pub fn can_do_turn(
        &mut self,
        req: Request,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
    ) -> bool {
        let i = map.get_t(req.turn).parent;

        match self.intersections[i.0] {
            IntersectionPolicy::StopSignPolicy(ref mut p) => {
                p.can_do_turn(req, time, map, control_map)
            }
            IntersectionPolicy::TrafficSignalPolicy(ref mut p) => {
                p.can_do_turn(req, time, map, control_map)
            }
        }
    }

    pub fn on_enter(&self, req: Request, map: &Map) {
        let i = map.get_t(req.turn).parent;

        match self.intersections[i.0] {
            IntersectionPolicy::StopSignPolicy(ref p) => p.on_enter(req),
            IntersectionPolicy::TrafficSignalPolicy(ref p) => p.on_enter(req),
        }
    }

    pub fn on_exit(&mut self, req: Request, map: &Map) {
        let i = map.get_t(req.turn).parent;

        match self.intersections[i.0] {
            IntersectionPolicy::StopSignPolicy(ref mut p) => p.on_exit(req),
            IntersectionPolicy::TrafficSignalPolicy(ref mut p) => p.on_exit(req),
        }
    }
}

// Use an enum instead of traits so that serialization works. I couldn't figure out erased_serde.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum IntersectionPolicy {
    StopSignPolicy(StopSign),
    TrafficSignalPolicy(TrafficSignal),
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct StopSign {
    id: IntersectionID,
    // Use BTreeMap so serialized state is easy to compare.
    // https://stackoverflow.com/questions/42723065/how-to-sort-hashmap-keys-when-serializing-with-serde
    // is an alt.
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    started_waiting_at: BTreeMap<AgentID, Tick>,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    accepted: BTreeMap<AgentID, TurnID>,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    waiting: BTreeMap<AgentID, TurnID>,
}

impl StopSign {
    fn new(id: IntersectionID) -> StopSign {
        StopSign {
            id,
            started_waiting_at: BTreeMap::new(),
            accepted: BTreeMap::new(),
            waiting: BTreeMap::new(),
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
        req: Request,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
    ) -> bool {
        let (agent, turn) = (req.agent, req.turn);
        assert_eq!(map.get_t(turn).parent, self.id);

        if self.accepted.contains_key(&agent) {
            return true;
        }

        if !self.started_waiting_at.contains_key(&agent) {
            self.started_waiting_at.insert(agent, time);
        }

        if self.conflicts_with_accepted(turn, map) {
            self.waiting.insert(agent, turn);
            return false;
        }

        let ss = &control_map.stop_signs[&self.id];
        if self.conflicts_with_waiting_with_higher_priority(turn, map, ss) {
            self.waiting.insert(agent, turn);
            return false;
        }
        if ss.get_priority(turn) == TurnPriority::Stop
            && (time - self.started_waiting_at[&agent]).as_time() < WAIT_AT_STOP_SIGN
        {
            self.waiting.insert(agent, turn);
            return false;
        }

        self.accepted.insert(agent, turn);
        self.waiting.remove(&agent);
        self.started_waiting_at.remove(&agent);
        true
    }

    fn on_enter(&self, req: Request) {
        assert!(self.accepted.contains_key(&req.agent));
    }

    fn on_exit(&mut self, req: Request) {
        assert!(self.accepted.contains_key(&req.agent));
        self.accepted.remove(&req.agent);
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct TrafficSignal {
    id: IntersectionID,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    accepted: BTreeMap<AgentID, TurnID>,
}

impl TrafficSignal {
    fn new(id: IntersectionID) -> TrafficSignal {
        TrafficSignal {
            id,
            accepted: BTreeMap::new(),
        }
    }

    // TODO determine if agents are staying in the intersection past the cycle time.

    fn can_do_turn(
        &mut self,
        req: Request,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
    ) -> bool {
        let turn = map.get_t(req.turn);

        assert_eq!(turn.parent, self.id);

        if self.accepted.contains_key(&req.agent) {
            return true;
        }

        let signal = &control_map.traffic_signals[&self.id];
        let (cycle, remaining_cycle_time) = signal.current_cycle_and_remaining_time(time.as_time());

        if !cycle.contains(turn.id) {
            return false;
        }
        // How long will it take the agent to cross the turn?
        // TODO different speeds
        let crossing_time = turn.length() / SPEED_LIMIT;
        // TODO account for TIMESTEP

        if crossing_time < remaining_cycle_time {
            self.accepted.insert(req.agent, turn.id);
            return true;
        }

        false
    }

    fn on_enter(&self, req: Request) {
        assert!(self.accepted.contains_key(&req.agent));
    }

    fn on_exit(&mut self, req: Request) {
        assert!(self.accepted.contains_key(&req.agent));
        self.accepted.remove(&req.agent);
    }
}
