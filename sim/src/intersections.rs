use crate::view::WorldView;
use crate::{AgentID, CarID, Event, PedestrianID, Tick, TIMESTEP};
use abstutil;
use abstutil::{deserialize_btreemap, serialize_btreemap, Error};
use geom::Duration;
use map_model::{
    ControlStopSign, IntersectionID, IntersectionType, LaneID, Map, TurnID, TurnPriority,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const WAIT_AT_STOP_SIGN: Duration = Duration::const_seconds(1.5);

// One agent may make several requests at one intersection at a time. This is normal for
// pedestrians and crosswalks. IntersectionPolicies should expect this.
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

#[derive(Serialize, Deserialize, PartialEq)]
pub struct IntersectionSimState {
    intersections: Vec<IntersectionPolicy>,
    debug: Option<IntersectionID>,
}

impl IntersectionSimState {
    pub fn new(map: &Map) -> IntersectionSimState {
        let mut intersections: Vec<IntersectionPolicy> = Vec::new();
        for i in map.all_intersections() {
            intersections.push(match i.intersection_type {
                IntersectionType::StopSign => IntersectionPolicy::StopSign(StopSign::new(i.id)),
                IntersectionType::TrafficSignal => {
                    IntersectionPolicy::TrafficSignal(TrafficSignal::new(i.id))
                }
                IntersectionType::Border => IntersectionPolicy::Border,
            });
        }
        IntersectionSimState {
            intersections,
            debug: None,
        }
    }

    // This is just an immutable query.
    pub fn request_granted(&self, req: Request) -> bool {
        let i = &self.intersections[req.turn.parent.0];
        i.is_accepted(&req)
    }

    // This is mutable, but MUST be idempotent, because it could be called in parallel/nondet
    // orders. It does NOT grant the request, just enqueues it for later consideration. The agent
    // MIGHT NOT be ready to enter the intersection (lookahead could send the request before the
    // agent is the leader vehicle and at the end of the lane). The request may have been
    // previously granted, but the agent might not have been able to start the turn.
    pub fn submit_request(&mut self, req: Request) {
        let i = &mut self.intersections[req.turn.parent.0];
        if i.is_accepted(&req) {
            return;
        }
        match i {
            IntersectionPolicy::StopSign(ref mut p) => {
                if !p.started_waiting_at.contains_key(&req) {
                    p.approaching_agents.insert(req);
                }
            }
            IntersectionPolicy::TrafficSignal(ref mut p) => {
                p.requests.insert(req);
            }
            IntersectionPolicy::Border => {}
        }
    }

    pub fn step(&mut self, events: &mut Vec<Event>, time: Tick, map: &Map, view: &WorldView) {
        for i in self.intersections.iter_mut() {
            match i {
                IntersectionPolicy::StopSign(ref mut p) => p.step(events, time, map, view),
                IntersectionPolicy::TrafficSignal(ref mut p) => p.step(events, time, map, view),
                IntersectionPolicy::Border => {}
            }
        }
    }

    pub fn on_enter(&self, req: Request) -> Result<(), Error> {
        let id = req.turn.parent;
        let i = &self.intersections[id.0];
        if i.is_accepted(&req) {
            if self.debug == Some(id) {
                debug!("{:?} just entered", req);
            }
            Ok(())
        } else {
            Err(Error::new(format!(
                "{:?} entered, but wasn't accepted by the intersection yet",
                req
            )))
        }
    }

    pub fn on_exit(&mut self, req: Request) {
        let id = req.turn.parent;
        let i = &mut self.intersections[id.0];
        assert!(i.is_accepted(&req));
        i.on_exit(&req);
        if self.debug == Some(id) {
            debug!("{:?} just exited", req);
        }
    }

    pub fn debug(&mut self, id: IntersectionID, map: &Map) {
        if let Some(old) = self.debug {
            match self.intersections[old.0] {
                IntersectionPolicy::StopSign(ref mut p) => {
                    p.debug = false;
                }
                IntersectionPolicy::TrafficSignal(ref mut p) => {
                    p.debug = false;
                }
                IntersectionPolicy::Border => {}
            };
        }

        println!("{}", abstutil::to_json(&self.intersections[id.0]));
        match self.intersections[id.0] {
            IntersectionPolicy::StopSign(ref mut p) => {
                p.debug = true;
                println!("{}", abstutil::to_json(map.get_stop_sign(id)));
            }
            IntersectionPolicy::TrafficSignal(ref mut p) => {
                p.debug = true;
                println!("{}", abstutil::to_json(map.get_traffic_signal(id)));
            }
            IntersectionPolicy::Border => {}
        };
    }

    pub fn get_accepted_agents(&self, id: IntersectionID) -> HashSet<AgentID> {
        match self.intersections[id.0] {
            IntersectionPolicy::StopSign(ref p) => p.accepted.iter().map(|req| req.agent).collect(),
            IntersectionPolicy::TrafficSignal(ref p) => {
                p.accepted.iter().map(|req| req.agent).collect()
            }
            // Technically anybody on incoming lanes
            IntersectionPolicy::Border => HashSet::new(),
        }
    }

    pub fn is_in_overtime(&self, id: IntersectionID) -> bool {
        match self.intersections[id.0] {
            IntersectionPolicy::StopSign(_) => unreachable!(),
            IntersectionPolicy::TrafficSignal(ref p) => p.overtime,
            IntersectionPolicy::Border => unreachable!(),
        }
    }

    pub fn anybody_accepted_with_destination(&self, i: IntersectionID, id: LaneID) -> bool {
        match self.intersections[i.0] {
            IntersectionPolicy::StopSign(ref p) => p.accepted.iter().any(|req| req.turn.dst == id),
            IntersectionPolicy::TrafficSignal(ref p) => {
                p.accepted.iter().any(|req| req.turn.dst == id)
            }
            IntersectionPolicy::Border => unreachable!(),
        }
    }
}

// Use an enum instead of traits so that serialization works. I couldn't figure out erased_serde.
// TODO check out https://github.com/dtolnay/typetag
#[derive(Serialize, Deserialize, PartialEq)]
enum IntersectionPolicy {
    StopSign(StopSign),
    TrafficSignal(TrafficSignal),
    Border,
}

impl IntersectionPolicy {
    fn is_accepted(&self, req: &Request) -> bool {
        match self {
            IntersectionPolicy::StopSign(ref p) => p.accepted.contains(req),
            IntersectionPolicy::TrafficSignal(ref p) => p.accepted.contains(req),
            IntersectionPolicy::Border => true,
        }
    }

    fn on_exit(&mut self, req: &Request) {
        match self {
            IntersectionPolicy::StopSign(ref mut p) => p.accepted.remove(&req),
            IntersectionPolicy::TrafficSignal(ref mut p) => p.accepted.remove(&req),
            IntersectionPolicy::Border => {
                panic!("{:?} called on_exit for a border node; how?!", req);
            }
        };
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct StopSign {
    id: IntersectionID,
    // Might not be stopped yet
    approaching_agents: BTreeSet<Request>,
    // Use BTreeMap so serialized state is easy to compare.
    // https://stackoverflow.com/questions/42723065/how-to-sort-hashmap-keys-when-serializing-with-serde
    // is an alt.
    // This is when the agent actually stopped.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    started_waiting_at: BTreeMap<Request, Tick>,
    accepted: BTreeSet<Request>,

    debug: bool,
}

impl StopSign {
    fn new(id: IntersectionID) -> StopSign {
        StopSign {
            id,
            approaching_agents: BTreeSet::new(),
            started_waiting_at: BTreeMap::new(),
            accepted: BTreeSet::new(),
            debug: false,
        }
    }

    fn conflicts_with_waiting_with_higher_priority(
        &self,
        turn: TurnID,
        map: &Map,
        ss: &ControlStopSign,
    ) -> bool {
        let base_t = map.get_t(turn);
        let base_priority = ss.get_priority(turn);
        self.started_waiting_at.keys().any(|req| {
            base_t.conflicts_with(map.get_t(req.turn)) && ss.get_priority(req.turn) > base_priority
        })
    }

    fn step(&mut self, events: &mut Vec<Event>, time: Tick, map: &Map, view: &WorldView) {
        let ss = map.get_stop_sign(self.id);

        // If anybody is stopped, promote them.
        // TODO retain() would rock
        let mut newly_stopped: Vec<Request> = Vec::new();
        for req in self.approaching_agents.iter() {
            // TODO or not blocked by somebody unaccepted
            if !view.is_leader(req.agent) {
                continue;
            }

            let should_promote = if ss.get_priority(req.turn) == TurnPriority::Stop {
                // TODO and the agent is at the end? maybe easier than looking at their speed
                // TODO with lane-changing, somebody could cut in front of them when they're stopped.
                view.get_speed(req.agent).is_zero(TIMESTEP)
            } else {
                true
            };
            if should_promote {
                self.started_waiting_at.insert(req.clone(), time);
                newly_stopped.push(req.clone());
                if self.debug {
                    debug!("{:?} is promoted from approaching to waiting", req);
                }
            }
        }
        for req in newly_stopped.into_iter() {
            self.approaching_agents.remove(&req);
        }

        let mut newly_accepted: Vec<Request> = Vec::new();
        for (req, started_waiting) in self.started_waiting_at.iter() {
            assert_eq!(req.turn.parent, self.id);
            assert_eq!(self.accepted.contains(&req), false);

            if conflicts_with_accepted(&self.accepted, req.turn, map) {
                continue;
            }

            if self.conflicts_with_waiting_with_higher_priority(req.turn, map, ss) {
                continue;
            }
            if ss.get_priority(req.turn) == TurnPriority::Stop
                && (time - *started_waiting).as_time() < WAIT_AT_STOP_SIGN
            {
                continue;
            }

            newly_accepted.push(req.clone());
            self.accepted.insert(req.clone());
            if self.debug {
                debug!("{:?} has been approved", req);
            }
        }

        for req in newly_accepted.into_iter() {
            self.started_waiting_at.remove(&req);
            events.push(Event::IntersectionAcceptsRequest(req));
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct TrafficSignal {
    id: IntersectionID,
    accepted: BTreeSet<Request>,
    requests: BTreeSet<Request>,
    overtime: bool,
    debug: bool,
}

impl TrafficSignal {
    fn new(id: IntersectionID) -> TrafficSignal {
        TrafficSignal {
            id,
            accepted: BTreeSet::new(),
            requests: BTreeSet::new(),
            overtime: false,
            debug: false,
        }
    }

    fn step(&mut self, events: &mut Vec<Event>, time: Tick, map: &Map, view: &WorldView) {
        let signal = map.get_traffic_signal(self.id);
        let (cycle, _remaining_cycle_time) =
            signal.current_cycle_and_remaining_time(time.as_time());

        // For now, just maintain safety when agents over-run.
        for req in self.accepted.iter() {
            if cycle.get_priority(req.turn) < TurnPriority::Yield {
                if self.debug {
                    debug!(
                        "{:?} is still doing {:?} after the cycle is over",
                        req.agent, req.turn
                    );
                }
                self.overtime = true;
                return;
            }
        }
        self.overtime = false;

        let priority_requests: BTreeSet<TurnID> = self
            .requests
            .iter()
            .filter_map(|req| {
                if cycle.get_priority(req.turn) == TurnPriority::Priority {
                    Some(req.turn)
                } else {
                    None
                }
            })
            .collect();

        let mut keep_requests: BTreeSet<Request> = BTreeSet::new();
        for req in self.requests.iter() {
            assert_eq!(req.turn.parent, self.id);
            assert_eq!(self.accepted.contains(&req), false);

            // Can't go at all this cycle.
            if cycle.get_priority(req.turn) < TurnPriority::Yield
                // Don't accept cars unless they're in front. TODO or behind other accepted cars.
                || !view.is_leader(req.agent)
                || conflicts_with_accepted(&self.accepted, req.turn, map)
            {
                keep_requests.insert(req.clone());
                continue;
            }

            // If there's a conflicting Priority request, don't go, even if that Priority
            // request can't go right now (due to a conflicting previously-accepted accepted
            // Yield).
            if cycle.get_priority(req.turn) == TurnPriority::Yield {
                let base_t = map.get_t(req.turn);
                if priority_requests
                    .iter()
                    .any(|t| base_t.conflicts_with(map.get_t(*t)))
                {
                    keep_requests.insert(req.clone());
                    continue;
                }
            }

            // TODO Don't accept agents if they won't make the light. But calculating that is
            // hard...
            //let crossing_time = turn.length() / speeds[&agent];

            self.accepted.insert(req.clone());
            events.push(Event::IntersectionAcceptsRequest(req.clone()));

            if self.debug {
                debug!("{:?} has been accepted for this cycle", req);
            }
        }

        self.requests = keep_requests;
    }
}

fn conflicts_with_accepted(accepted: &BTreeSet<Request>, turn: TurnID, map: &Map) -> bool {
    let base_t = map.get_t(turn);
    accepted
        .iter()
        .any(|req| base_t.conflicts_with(map.get_t(req.turn)))
}
