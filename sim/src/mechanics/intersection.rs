use crate::AgentID;
use geom::Duration;
use map_model::{ControlTrafficSignal, IntersectionID, LaneID, Map, TurnID, TurnPriority};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct IntersectionSimState {
    controllers: BTreeMap<IntersectionID, IntersectionController>,
}

impl IntersectionSimState {
    pub fn new(map: &Map) -> IntersectionSimState {
        let mut sim = IntersectionSimState {
            controllers: BTreeMap::new(),
        };
        for i in map.all_intersections() {
            sim.controllers.insert(
                i.id,
                IntersectionController {
                    id: i.id,
                    accepted: BTreeSet::new(),
                    waiting: BTreeSet::new(),
                },
            );
        }
        sim
    }

    pub fn nobody_headed_towards(&self, lane: LaneID, i: IntersectionID) -> bool {
        self.controllers[&i].nobody_headed_towards(lane)
    }

    pub fn turn_finished(&mut self, agent: AgentID, turn: TurnID) {
        self.controllers
            .get_mut(&turn.parent)
            .unwrap()
            .turn_finished(agent, turn);
    }

    // TODO This API is bad. Need to gather all of the requests at a time before making a decision.
    pub fn maybe_start_turn(
        &mut self,
        agent: AgentID,
        turn: TurnID,
        time: Duration,
        map: &Map,
    ) -> bool {
        self.controllers
            .get_mut(&turn.parent)
            .unwrap()
            .maybe_start_turn(agent, turn, time, map)
    }

    pub fn debug(&self, id: IntersectionID, map: &Map) {
        println!("{}", abstutil::to_json(&self.controllers[&id]));
        if let Some(ref sign) = map.maybe_get_stop_sign(id) {
            println!("{}", abstutil::to_json(sign));
        } else if let Some(ref signal) = map.maybe_get_traffic_signal(id) {
            println!("{}", abstutil::to_json(signal));
        } else {
            println!("Border");
        }
    }

    pub fn get_accepted_agents(&self, id: IntersectionID) -> HashSet<AgentID> {
        self.controllers[&id]
            .accepted
            .iter()
            .map(|req| req.agent)
            .collect()
    }

    pub fn is_in_overtime(&self, time: Duration, id: IntersectionID, map: &Map) -> bool {
        if let Some(ref signal) = map.maybe_get_traffic_signal(id) {
            let (cycle, _) = signal.current_cycle_and_remaining_time(time);
            self.controllers[&id]
                .accepted
                .iter()
                .any(|req| cycle.get_priority(req.turn) < TurnPriority::Yield)
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct IntersectionController {
    id: IntersectionID,
    accepted: BTreeSet<Request>,
    waiting: BTreeSet<Request>,
}

impl IntersectionController {
    // For cars: The head car calls this when they're at the end of the lane Queued. If this returns true,
    // then the head car MUST actually start this turn.
    // For peds: Likewise -- only called when the ped is at the start of the turn. They must
    // actually do the turn if this returns true.
    fn maybe_start_turn(
        &mut self,
        agent: AgentID,
        turn: TurnID,
        time: Duration,
        map: &Map,
    ) -> bool {
        let req = Request { agent, turn };
        let allowed = if let Some(ref signal) = map.maybe_get_traffic_signal(self.id) {
            self.traffic_signal_policy(signal, &req, time, map)
        } else {
            self.freeform_policy(&req, time, map)
        };
        if allowed {
            assert!(!self.any_accepted_conflict_with(turn, map));
            self.waiting.remove(&req);
            self.accepted.insert(req);
            true
        } else {
            self.waiting.insert(req);
            false
        }
    }

    fn nobody_headed_towards(&self, dst_lane: LaneID) -> bool {
        !self.accepted.iter().any(|req| req.turn.dst == dst_lane)
    }

    fn turn_finished(&mut self, agent: AgentID, turn: TurnID) {
        assert!(self.accepted.remove(&Request { agent, turn }));
    }

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
                println!(
                    "{:?} is still doing {:?} after the cycle is over",
                    req.agent, req.turn
                );
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
