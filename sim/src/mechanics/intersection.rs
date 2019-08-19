use crate::mechanics::car::Car;
use crate::mechanics::queue::Queue;
use crate::{AgentID, Command, Scheduler, Speed};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Duration;
use map_model::{
    ControlStopSign, ControlTrafficSignal, IntersectionID, IntersectionType, LaneID, Map, TurnID,
    TurnPriority, TurnType,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const WAIT_AT_STOP_SIGN: Duration = Duration::const_seconds(0.5);

#[derive(Serialize, Deserialize, PartialEq)]
pub struct IntersectionSimState {
    state: BTreeMap<IntersectionID, State>,
    use_freeform_policy_everywhere: bool,
    force_queue_entry: bool,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct State {
    id: IntersectionID,
    accepted: BTreeSet<Request>,
    // Track when a request is first made.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    waiting: BTreeMap<Request, Duration>,
}

impl IntersectionSimState {
    pub fn new(
        map: &Map,
        scheduler: &mut Scheduler,
        use_freeform_policy_everywhere: bool,
        disable_block_the_box: bool,
    ) -> IntersectionSimState {
        let mut sim = IntersectionSimState {
            state: BTreeMap::new(),
            use_freeform_policy_everywhere,
            force_queue_entry: disable_block_the_box,
        };
        for i in map.all_intersections() {
            sim.state.insert(
                i.id,
                State {
                    id: i.id,
                    accepted: BTreeSet::new(),
                    waiting: BTreeMap::new(),
                },
            );
            if i.intersection_type == IntersectionType::TrafficSignal {
                sim.update_intersection(Duration::ZERO, i.id, map, scheduler);
            }
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
        now: Duration,
        agent: AgentID,
        turn: TurnID,
        scheduler: &mut Scheduler,
    ) {
        let state = self.state.get_mut(&turn.parent).unwrap();

        assert!(state.accepted.remove(&Request { agent, turn }));

        // TODO Could be smarter here. For both policies, only wake up agents that would then be
        // accepted. For now, wake up everyone -- for traffic signals, maybe a Yield and Priority
        // finished and could let another one in.

        for req in state.waiting.keys() {
            // Use update because multiple agents could finish a turn at the same time, before the
            // waiting one has a chance to try again.
            scheduler.update(now, Command::update_agent(req.agent));
        }
    }

    pub fn space_freed(&mut self, now: Duration, i: IntersectionID, scheduler: &mut Scheduler) {
        let state = self.state.get_mut(&i).unwrap();

        // TODO Be smarter and wake up less people here too.

        for req in state.waiting.keys() {
            // Use update for similar reasons to turn_finished.
            scheduler.update(now, Command::update_agent(req.agent));
        }
    }

    // This is only triggered for traffic signals.
    pub fn update_intersection(
        &self,
        now: Duration,
        id: IntersectionID,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        let state = &self.state[&id];
        let (_, _, remaining) = map
            .get_traffic_signal(id)
            .current_cycle_and_remaining_time(now);

        // TODO Wake up everyone, for now.
        // TODO Use update in case turn_finished scheduled an event for them already.
        for req in state.waiting.keys() {
            scheduler.update(now, Command::update_agent(req.agent));
        }

        scheduler.push(now + remaining, Command::UpdateIntersection(id));
    }

    // For cars: The head car calls this when they're at the end of the lane WaitingToAdvance. If
    // this returns true, then the head car MUST actually start this turn.
    // For peds: Likewise -- only called when the ped is at the start of the turn. They must
    // actually do the turn if this returns true.
    //
    // If this returns false, the agent should NOT retry. IntersectionSimState will schedule a
    // retry event at some point.
    pub fn maybe_start_turn(
        &mut self,
        agent: AgentID,
        turn: TurnID,
        speed: Speed,
        now: Duration,
        map: &Map,
        scheduler: &mut Scheduler,
        maybe_car_and_target_queue: Option<(&mut Queue, &Car)>,
    ) -> bool {
        let req = Request { agent, turn };
        let state = self.state.get_mut(&turn.parent).unwrap();
        state.waiting.entry(req.clone()).or_insert(now);

        let allowed = if self.use_freeform_policy_everywhere {
            state.freeform_policy(&req, map)
        } else if let Some(ref signal) = map.maybe_get_traffic_signal(state.id) {
            state.traffic_signal_policy(signal, &req, speed, now, map)
        } else if let Some(ref sign) = map.maybe_get_stop_sign(state.id) {
            state.stop_sign_policy(sign, &req, now, map, scheduler)
        } else {
            unreachable!()
        };
        if !allowed {
            return false;
        }

        // Don't block the box
        if let Some((queue, car)) = maybe_car_and_target_queue {
            if !queue.try_to_reserve_entry(car, self.force_queue_entry) {
                return false;
            }
        }

        assert!(!state.any_accepted_conflict_with(turn, map));
        state.waiting.remove(&req).unwrap();
        state.accepted.insert(req);
        true
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
}

impl State {
    fn any_accepted_conflict_with(&self, t: TurnID, map: &Map) -> bool {
        let turn = map.get_t(t);
        self.accepted
            .iter()
            .any(|req| map.get_t(req.turn).conflicts_with(turn))
    }

    fn freeform_policy(&self, req: &Request, map: &Map) -> bool {
        // Allow concurrent turns that don't conflict
        if self.any_accepted_conflict_with(req.turn, map) {
            return false;
        }

        true
    }

    fn stop_sign_policy(
        &self,
        sign: &ControlStopSign,
        req: &Request,
        now: Duration,
        map: &Map,
        scheduler: &mut Scheduler,
    ) -> bool {
        if self.any_accepted_conflict_with(req.turn, map) {
            return false;
        }

        let our_priority = sign.turns[&req.turn];
        assert!(our_priority != TurnPriority::Banned);
        let our_time = self.waiting[req];

        if our_priority == TurnPriority::Stop && now < our_time + WAIT_AT_STOP_SIGN {
            // Since we have "ownership" of scheduling for req.agent, don't need to use
            // scheduler.update.
            scheduler.push(
                our_time + WAIT_AT_STOP_SIGN,
                Command::update_agent(req.agent),
            );
            return false;
        }

        // Once upon a time, we'd make sure that this request doesn't conflict with another in
        // self.waiting:
        // 1) Higher-ranking turns get to go first.
        // 2) Equal-ranking turns that started waiting before us get to go first.
        // But the exceptions started stacking -- if the other agent is blocked or the turns don't
        // even conflict, then allow it. Except determining if the other agent is blocked or not is
        // tough and kind of recursive.
        //
        // So instead, don't do any of that! The WAIT_AT_STOP_SIGN scheduling above and the fact
        // that events are processed in time order mean that case #2 is magically handled anyway.
        // If a case #1 could've started by now, then they would have. Since they didn't, they must
        // be blocked.

        // TODO Make sure we can optimistically finish this turn before an approaching
        // higher-priority vehicle wants to begin.

        true
    }

    fn traffic_signal_policy(
        &self,
        signal: &ControlTrafficSignal,
        new_req: &Request,
        speed: Speed,
        time: Duration,
        map: &Map,
    ) -> bool {
        let turn = map.get_t(new_req.turn);

        // SharedSidewalkCorner doesn't conflict with anything -- fastpath!
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            return true;
        }

        let (_, cycle, remaining_cycle_time) = signal.current_cycle_and_remaining_time(time);

        // Can't go at all this cycle.
        if cycle.get_priority(new_req.turn) == TurnPriority::Banned {
            return false;
        }

        // Somebody might already be doing a Yield turn that conflicts with this one.
        if self.any_accepted_conflict_with(new_req.turn, map) {
            return false;
        }

        // A yield loses to a conflicting Priority turn.
        if cycle.get_priority(new_req.turn) == TurnPriority::Yield {
            if self.waiting.keys().any(|r| {
                turn.conflicts_with(map.get_t(r.turn))
                    && cycle.get_priority(r.turn) == TurnPriority::Priority
            }) {
                return false;
            }
        }

        // TODO Make sure we can optimistically finish this turn before an approaching
        // higher-priority vehicle wants to begin.

        // Optimistically if nobody else is in the way, this is how long it'll take to finish the
        // turn. Don't start the turn if we won't finish by the time the light changes. If we get
        // it wrong, that's fine -- block the box a bit.
        let time_to_cross = turn.geom.length() / speed;
        if time_to_cross > remaining_cycle_time {
            // Actually, we might have bigger problems...
            if time_to_cross > cycle.duration {
                println!("OYYY! {:?} is impossible to fit into cycle duration of {}. Allowing, but fix the policy!", new_req, cycle.duration);
            } else {
                return false;
            }
        }

        true
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone, Debug)]
struct Request {
    agent: AgentID,
    turn: TurnID,
}
