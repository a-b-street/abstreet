use crate::plugins::sim::new_des_model::Queue;
use geom::Duration;
use map_model::{
    ControlTrafficSignal, IntersectionID, LaneID, Map, Traversable, TurnID, TurnPriority,
};
use sim::AgentID;
use std::collections::{BTreeMap, HashSet};

pub struct IntersectionSimState {
    controllers: BTreeMap<IntersectionID, IntersectionController>,
}

impl IntersectionSimState {
    pub fn new(map: &Map) -> IntersectionSimState {
        let mut sim = IntersectionSimState {
            controllers: BTreeMap::new(),
        };
        for i in map.all_intersections() {
            sim.controllers
                .insert(i.id, IntersectionController::new(i.id));
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

    pub fn maybe_start_turn(
        &mut self,
        agent: AgentID,
        turn: TurnID,
        queues: &BTreeMap<Traversable, Queue>,
        time: Duration,
        map: &Map,
    ) -> bool {
        self.controllers
            .get_mut(&turn.parent)
            .unwrap()
            .maybe_start_turn(agent, turn, queues, time, map)
    }
}

struct IntersectionController {
    id: IntersectionID,
    accepted: HashSet<Request>,
}

impl IntersectionController {
    fn new(id: IntersectionID) -> IntersectionController {
        IntersectionController {
            id,
            accepted: HashSet::new(),
        }
    }

    // The head car calls this when they're at the end of the lane Queued. If this returns true,
    // then the head car MUST actually start this turn.
    // TODO And how bout for peds?
    fn maybe_start_turn(
        &mut self,
        agent: AgentID,
        turn: TurnID,
        queues: &BTreeMap<Traversable, Queue>,
        time: Duration,
        map: &Map,
    ) -> bool {
        // Policy: only one turn at a time, can't go until the target lane has room.
        /*if !self.accepted.is_empty() {
            return false;
        }
        // TODO This isn't strong enough -- make sure there's room for the car to immediately
        // complete the turn and get out of the intersection completely.
        if !queues[&Traversable::Lane(turn.dst)].room_at_end(time) {
            return false;
        }*/

        let allowed = if let Some(ref signal) = map.maybe_get_traffic_signal(self.id) {
            self.traffic_signal_policy(signal, agent, turn, queues, time, map)
        } else {
            self.freeform_policy(agent, turn, queues, time, map)
        };
        if allowed {
            assert!(!self.any_accepted_conflict_with(turn, map));
            self.accepted.insert(Request { agent, turn });
        }
        allowed
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

    fn freeform_policy(
        &self,
        _agent: AgentID,
        t: TurnID,
        _queues: &BTreeMap<Traversable, Queue>,
        _time: Duration,
        map: &Map,
    ) -> bool {
        // Allow concurrent turns that don't conflict, don't prevent target lane from spilling
        // over.
        if self.any_accepted_conflict_with(t, map) {
            return false;
        }
        true
    }

    fn traffic_signal_policy(
        &self,
        signal: &ControlTrafficSignal,
        _agent: AgentID,
        turn: TurnID,
        _queues: &BTreeMap<Traversable, Queue>,
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
        if cycle.get_priority(turn) < TurnPriority::Yield {
            return false;
        }

        // Somebody might already be doing a Yield turn that conflicts with this one.
        if self.any_accepted_conflict_with(turn, map) {
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

#[derive(Hash, PartialEq, Eq)]
struct Request {
    agent: AgentID,
    turn: TurnID,
}
