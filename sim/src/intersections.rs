// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil::{deserialize_btreemap, serialize_btreemap};
use control::stop_signs::{ControlStopSign, TurnPriority};
use control::ControlMap;
use dimensioned::si;
use kinematics;
use map_model::{IntersectionID, Map, TurnID};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use {AgentID, CarID, InvariantViolated, PedestrianID, Speed, Tick, Time};

use std;
const WAIT_AT_STOP_SIGN: Time = si::Second {
    value_unsafe: 1.5,
    _marker: std::marker::PhantomData,
};

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
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

    // This is just an immutable query.
    pub fn request_granted(&self, req: Request) -> bool {
        let i = &self.intersections[req.turn.parent.0];
        i.accepted().get(&req.agent) == Some(&req.turn)
    }

    // This is mutable, but MUST be idempotent, because it could be called in parallel/nondet
    // orders. It does NOT grant the request, just enqueues it for later consideration. The agent
    // must be ready to enter the intersection (leader vehicle and at the end of the lane already).
    // The request may have been previously granted, but the agent might not have been able to
    // start the turn.
    pub fn submit_request(&mut self, req: Request) -> Result<(), InvariantViolated> {
        let i = self.intersections.get_mut(req.turn.parent.0).unwrap();
        if let Some(t) = i.accepted().get(&req.agent) {
            if *t != req.turn {
                return Err(InvariantViolated(format!(
                    "{:?} made, but {} has already been accepted",
                    req, t
                )));
            }
            return Ok(());
        }
        match i {
            IntersectionPolicy::StopSignPolicy(ref mut p) => {
                // TODO assert that the agent hasn't requested something different previously
                if !p.started_waiting_at.contains_key(&req) {
                    p.approaching_agents.insert(req);
                }
            }
            IntersectionPolicy::TrafficSignalPolicy(ref mut p) => {
                // TODO assert that the agent hasn't requested something different previously
                p.requests.insert(req);
            }
        }
        Ok(())
    }

    pub fn step(
        &mut self,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
        speeds: HashMap<AgentID, Speed>,
    ) {
        for i in self.intersections.iter_mut() {
            match i {
                IntersectionPolicy::StopSignPolicy(ref mut p) => {
                    p.step(time, map, control_map, &speeds)
                }
                IntersectionPolicy::TrafficSignalPolicy(ref mut p) => {
                    p.step(time, map, control_map, &speeds)
                }
            }
        }
    }

    pub fn on_enter(&self, req: Request) -> Result<(), InvariantViolated> {
        let i = &self.intersections[req.turn.parent.0];
        if i.accepted().contains_key(&req.agent) {
            Ok(())
        } else {
            Err(InvariantViolated(format!(
                "{:?} entered, but wasn't accepted by the intersection yet",
                req
            )))
        }
    }

    pub fn on_exit(&mut self, req: Request) {
        let i = self.intersections.get_mut(req.turn.parent.0).unwrap();
        assert!(i.accepted().contains_key(&req.agent));
        i.accepted_mut().remove(&req.agent);
    }
}

// Use an enum instead of traits so that serialization works. I couldn't figure out erased_serde.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum IntersectionPolicy {
    StopSignPolicy(StopSign),
    TrafficSignalPolicy(TrafficSignal),
}

impl IntersectionPolicy {
    fn accepted(&self) -> &BTreeMap<AgentID, TurnID> {
        match self {
            IntersectionPolicy::StopSignPolicy(ref p) => &p.accepted,
            IntersectionPolicy::TrafficSignalPolicy(ref p) => &p.accepted,
        }
    }

    fn accepted_mut(&mut self) -> &mut BTreeMap<AgentID, TurnID> {
        match self {
            IntersectionPolicy::StopSignPolicy(ref mut p) => &mut p.accepted,
            IntersectionPolicy::TrafficSignalPolicy(ref mut p) => &mut p.accepted,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct StopSign {
    id: IntersectionID,
    // Might not be stopped yet
    approaching_agents: BTreeSet<Request>,
    // Use BTreeMap so serialized state is easy to compare.
    // https://stackoverflow.com/questions/42723065/how-to-sort-hashmap-keys-when-serializing-with-serde
    // is an alt.
    // This is when the agent actually stopped.
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    started_waiting_at: BTreeMap<Request, Tick>,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    accepted: BTreeMap<AgentID, TurnID>,
}

impl StopSign {
    fn new(id: IntersectionID) -> StopSign {
        StopSign {
            id,
            approaching_agents: BTreeSet::new(),
            started_waiting_at: BTreeMap::new(),
            accepted: BTreeMap::new(),
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
        self.started_waiting_at
            .keys()
            .find(|req| {
                base_t.conflicts_with(map.get_t(req.turn))
                    && ss.get_priority(req.turn) > base_priority
            })
            .is_some()
    }

    fn step(
        &mut self,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
        speeds: &HashMap<AgentID, Speed>,
    ) {
        // If anybody is stopped, promote them.
        // TODO retain() would rock
        let mut newly_stopped: Vec<Request> = Vec::new();
        for req in self.approaching_agents.iter() {
            // TODO tmpish debug
            if !speeds.contains_key(&req.agent) {
                println!("no speed for {:?}", req);
            }

            if speeds[&req.agent] <= kinematics::EPSILON_SPEED {
                self.started_waiting_at.insert(req.clone(), time);
                newly_stopped.push(req.clone());
            }
        }
        for req in newly_stopped.into_iter() {
            self.approaching_agents.remove(&req);
        }

        let mut newly_accepted: Vec<Request> = Vec::new();
        for (req, started_waiting) in self.started_waiting_at.iter() {
            let (agent, turn) = (req.agent, req.turn);
            assert_eq!(turn.parent, self.id);
            assert_eq!(self.accepted.contains_key(&agent), false);

            if self.conflicts_with_accepted(turn, map) {
                continue;
            }

            let ss = &control_map.stop_signs[&self.id];
            if self.conflicts_with_waiting_with_higher_priority(turn, map, ss) {
                continue;
            }
            if ss.get_priority(turn) == TurnPriority::Stop
                && (time - *started_waiting).as_time() < WAIT_AT_STOP_SIGN
            {
                continue;
            }

            newly_accepted.push(req.clone());
        }

        for req in newly_accepted.into_iter() {
            self.accepted.insert(req.agent, req.turn);
            self.started_waiting_at.remove(&req);
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct TrafficSignal {
    id: IntersectionID,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    accepted: BTreeMap<AgentID, TurnID>,
    requests: BTreeSet<Request>,
}

impl TrafficSignal {
    fn new(id: IntersectionID) -> TrafficSignal {
        TrafficSignal {
            id,
            accepted: BTreeMap::new(),
            requests: BTreeSet::new(),
        }
    }

    // TODO determine if agents are staying in the intersection past the cycle time.

    fn step(
        &mut self,
        time: Tick,
        map: &Map,
        control_map: &ControlMap,
        speeds: &HashMap<AgentID, Speed>,
    ) {
        let signal = &control_map.traffic_signals[&self.id];
        let (cycle, remaining_cycle_time) = signal.current_cycle_and_remaining_time(time.as_time());

        let mut keep_requests: BTreeSet<Request> = BTreeSet::new();
        for req in self.requests.iter() {
            let turn = map.get_t(req.turn);
            let agent = req.agent;
            assert_eq!(turn.parent, self.id);
            assert_eq!(self.accepted.contains_key(&agent), false);

            if !cycle.contains(turn.id) {
                keep_requests.insert(req.clone());
                continue;
            }
            // How long will it take the agent to cross the turn?
            // TODO assuming they accelerate!
            let crossing_time = turn.length() / speeds[&agent];
            // TODO account for TIMESTEP

            if crossing_time < remaining_cycle_time {
                self.accepted.insert(req.agent, turn.id);
            } else {
                keep_requests.insert(req.clone());
            }
        }

        self.requests = keep_requests;
    }
}
