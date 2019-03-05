use crate::{AgentID, Command, Scheduler, BLIND_RETRY};
use geom::Duration;
use map_model::{ControlTrafficSignal, IntersectionID, LaneID, Map, TurnID, TurnPriority};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct IntersectionSimState {
    state: BTreeMap<IntersectionID, State>,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct State {
    id: IntersectionID,
    accepted: BTreeSet<Request>,
    waiting: Vec<Request>,
}

impl IntersectionSimState {
    pub fn new(map: &Map) -> IntersectionSimState {
        let mut sim = IntersectionSimState {
            state: BTreeMap::new(),
        };
        for i in map.all_intersections() {
            sim.state.insert(
                i.id,
                State {
                    id: i.id,
                    accepted: BTreeSet::new(),
                    waiting: Vec::new(),
                },
            );
        }
        sim
    }

    pub fn nobody_headed_towards(&self, lane: LaneID, i: IntersectionID) -> bool {
        !self.state[&i]
            .accepted
            .iter()
            .any(|req| req.turn.dst == lane)
    }

    pub fn turn_finished(
        &mut self,
        _now: Duration,
        agent: AgentID,
        turn: TurnID,
        _scheduler: &mut Scheduler,
        _map: &Map,
    ) {
        let state = self.state.get_mut(&turn.parent).unwrap();

        assert!(state.accepted.remove(&Request { agent, turn }));

        /*if map.maybe_get_traffic_signal(state.id).is_some() {
            // TODO If we were in overtime...
        } else {
            // For the freeform policy, give everyone a chance
            for req in &state.waiting {
                scheduler.push(
                    now,
                    match req.agent {
                        AgentID::Car(id) => Command::UpdateCar(id),
                        AgentID::Pedestrian(id) => Command::UpdatePed(id),
                    },
                );
            }
        }*/
    }

    // TODO This API is bad. Need to gather all of the requests at a time before making a decision.
    //
    // For cars: The head car calls this when they're at the end of the lane Queued. If this returns true,
    // then the head car MUST actually start this turn.
    // For peds: Likewise -- only called when the ped is at the start of the turn. They must
    // actually do the turn if this returns true.
    //
    // If this returns false, the agent should NOT retry. IntersectionSimState will schedule a
    // retry event at some point.
    pub fn maybe_start_turn(
        &mut self,
        agent: AgentID,
        turn: TurnID,
        now: Duration,
        map: &Map,
        scheduler: &mut Scheduler,
    ) -> bool {
        let state = self.state.get_mut(&turn.parent).unwrap();

        let req = Request { agent, turn };
        let allowed = if let Some(ref signal) = map.maybe_get_traffic_signal(state.id) {
            state.traffic_signal_policy(signal, &req, now, map)
        } else {
            state.freeform_policy(&req, now, map)
        };
        if allowed {
            assert!(!state.any_accepted_conflict_with(turn, map));
            if let Some(idx) = state.waiting.iter().position(|r| *r == req) {
                state.waiting.remove(idx);
            }
            state.accepted.insert(req);
            true
        } else {
            // TODO Don't do this here.
            scheduler.push(
                now + BLIND_RETRY,
                match req.agent {
                    AgentID::Car(id) => Command::UpdateCar(id),
                    AgentID::Pedestrian(id) => Command::UpdatePed(id),
                },
            );

            state.waiting.push(req);
            false
        }
    }

    pub fn debug(&self, id: IntersectionID, map: &Map) {
        println!("{}", abstutil::to_json(&self.state[&id]));
        if let Some(ref sign) = map.maybe_get_stop_sign(id) {
            println!("{}", abstutil::to_json(sign));
        } else if let Some(ref signal) = map.maybe_get_traffic_signal(id) {
            println!("{}", abstutil::to_json(signal));
        } else {
            println!("Border");
        }
    }

    pub fn get_accepted_agents(&self, id: IntersectionID) -> HashSet<AgentID> {
        self.state[&id]
            .accepted
            .iter()
            .map(|req| req.agent)
            .collect()
    }

    pub fn is_in_overtime(&self, time: Duration, id: IntersectionID, map: &Map) -> bool {
        if let Some(ref signal) = map.maybe_get_traffic_signal(id) {
            let (cycle, _) = signal.current_cycle_and_remaining_time(time);
            self.state[&id]
                .accepted
                .iter()
                .any(|req| cycle.get_priority(req.turn) < TurnPriority::Yield)
        } else {
            false
        }
    }
}

impl State {
    fn any_accepted_conflict_with(&self, t: TurnID, map: &Map) -> bool {
        let turn = map.get_t(t);
        self.accepted
            .iter()
            .any(|req| map.get_t(req.turn).conflicts_with(turn))
    }

    fn freeform_policy(&self, req: &Request, _time: Duration, map: &Map) -> bool {
        // Allow concurrent turns that don't conflict, don't prevent target lane from spilling
        // over.
        if self.any_accepted_conflict_with(req.turn, map) {
            return false;
        }
        true
    }

    fn traffic_signal_policy(
        &self,
        signal: &ControlTrafficSignal,
        new_req: &Request,
        time: Duration,
        map: &Map,
    ) -> bool {
        let (cycle, _remaining_cycle_time) = signal.current_cycle_and_remaining_time(time);

        // For now, just maintain safety when agents over-run.
        for req in &self.accepted {
            if cycle.get_priority(req.turn) < TurnPriority::Yield {
                /*println!(
                    "{:?} is still doing {:?} after the cycle is over",
                    req.agent, req.turn
                );*/
                return false;
            }
        }

        // Can't go at all this cycle.
        if cycle.get_priority(new_req.turn) < TurnPriority::Yield {
            return false;
        }

        // Somebody might already be doing a Yield turn that conflicts with this one.
        if self.any_accepted_conflict_with(new_req.turn, map) {
            return false;
        }

        // TODO If there's a choice between a Priority and Yield request, choose Priority. Need
        // batched requests to know -- that'll come later, once the walking sim is integrated.

        // TODO Don't accept the agent if they won't finish the turn in time. If the turn and
        // target lane were clear, we could calculate the time, but it gets hard. For now, allow
        // overtime. This is trivial for peds.

        true
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone)]
struct Request {
    agent: AgentID,
    turn: TurnID,
}
