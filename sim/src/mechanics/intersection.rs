use crate::mechanics::car::Car;
use crate::mechanics::Queue;
use crate::{AgentID, AlertLocation, CarID, Command, Event, Scheduler, Speed};
use abstutil::{deserialize_btreemap, retain_btreeset, serialize_btreemap};
use geom::{Duration, Time};
use map_model::{
    ControlStopSign, ControlTrafficSignal, IntersectionID, LaneID, Map, RoadID, Traversable,
    TurnID, TurnPriority, TurnType,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const WAIT_AT_STOP_SIGN: Duration = Duration::const_seconds(0.5);
const WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL: Duration = Duration::const_seconds(0.2);

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct IntersectionSimState {
    state: BTreeMap<IntersectionID, State>,
    use_freeform_policy_everywhere: bool,
    dont_block_the_box: bool,
    break_turn_conflict_cycles: bool,
    // (x, y) means x is blocked by y. It's a many-to-many relationship. TODO Better data
    // structure.
    blocked_by: BTreeSet<(CarID, CarID)>,
    events: Vec<Event>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct State {
    id: IntersectionID,
    accepted: BTreeSet<Request>,
    // Track when a request is first made.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    waiting: BTreeMap<Request, Time>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone, Debug)]
struct Request {
    agent: AgentID,
    turn: TurnID,
}

impl IntersectionSimState {
    pub fn new(
        map: &Map,
        scheduler: &mut Scheduler,
        use_freeform_policy_everywhere: bool,
        dont_block_the_box: bool,
        break_turn_conflict_cycles: bool,
    ) -> IntersectionSimState {
        let mut sim = IntersectionSimState {
            state: BTreeMap::new(),
            use_freeform_policy_everywhere,
            dont_block_the_box,
            break_turn_conflict_cycles,
            blocked_by: BTreeSet::new(),
            events: Vec::new(),
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
            if i.is_traffic_signal() && !use_freeform_policy_everywhere {
                sim.update_intersection(Time::START_OF_DAY, i.id, map, scheduler);
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
        now: Time,
        agent: AgentID,
        turn: TurnID,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        let state = self.state.get_mut(&turn.parent).unwrap();
        assert!(state.accepted.remove(&Request { agent, turn }));
        if map.get_t(turn).turn_type != TurnType::SharedSidewalkCorner {
            self.wakeup_waiting(now, turn.parent, scheduler, map);
        }
        if self.break_turn_conflict_cycles {
            if let AgentID::Car(car) = agent {
                retain_btreeset(&mut self.blocked_by, |(_, c)| *c != car);
            }
        }
    }

    // For deleting cars
    pub fn cancel_request(&mut self, agent: AgentID, turn: TurnID) {
        let state = self.state.get_mut(&turn.parent).unwrap();
        state.waiting.remove(&Request { agent, turn });
        if self.break_turn_conflict_cycles {
            if let AgentID::Car(car) = agent {
                retain_btreeset(&mut self.blocked_by, |(c1, c2)| *c1 != car && *c2 != car);
            }
        }
    }

    pub fn space_freed(
        &mut self,
        now: Time,
        i: IntersectionID,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        self.wakeup_waiting(now, i, scheduler, map);
    }

    // Vanished at border, stopped biking, etc -- a vehicle disappeared, and didn't have one last
    // turn.
    pub fn vehicle_gone(&mut self, car: CarID) {
        retain_btreeset(&mut self.blocked_by, |(c1, c2)| *c1 != car && *c2 != car);
    }

    fn wakeup_waiting(&self, now: Time, i: IntersectionID, scheduler: &mut Scheduler, map: &Map) {
        /*if i == IntersectionID(64) {
            println!("at {}: wakeup_waiting -----------------", now);
        }*/
        let mut all: Vec<(Request, Time)> = self.state[&i]
            .waiting
            .iter()
            .map(|(r, t)| (r.clone(), *t))
            .collect();
        // Sort by waiting time, so things like stop signs actually are first-come, first-served.
        all.sort_by_key(|(_, t)| *t);

        // Wake up Priority turns before Yield turns. Don't wake up Banned turns at all. This makes
        // sure priority vehicles should get the head-start, without blocking yield vehicles
        // unnecessarily.
        let mut protected = Vec::new();
        let mut yielding = Vec::new();

        if self.use_freeform_policy_everywhere {
            for (req, _) in all {
                protected.push(req);
            }
        } else if let Some(ref signal) = map.maybe_get_traffic_signal(i) {
            let (_, phase, _) = signal.current_phase_and_remaining_time(now);
            for (req, _) in all {
                match phase.get_priority_of_turn(req.turn, signal) {
                    TurnPriority::Protected => {
                        protected.push(req);
                    }
                    TurnPriority::Yield => {
                        yielding.push(req);
                    }
                    // No need to wake up
                    TurnPriority::Banned => {}
                }
            }
        } else if let Some(ref sign) = map.maybe_get_stop_sign(i) {
            for (req, _) in all {
                // Banned is impossible
                if sign.get_priority(req.turn, map) == TurnPriority::Protected {
                    protected.push(req);
                } else {
                    yielding.push(req);
                }
            }
        } else {
            assert!(map.get_i(i).is_border());
        };

        for req in protected {
            // Use update because multiple agents could finish a turn at the same time, before the
            // waiting one has a chance to try again.
            scheduler.update(now, Command::update_agent(req.agent));
        }
        // Make sure the protected group gets first dibs. The scheduler arbitrarily (but
        // deterministically) orders commands with the same time.
        for req in yielding {
            scheduler.update(
                now + Duration::seconds(0.1),
                Command::update_agent(req.agent),
            );
        }
    }

    // This is only triggered for traffic signals.
    pub fn update_intersection(
        &self,
        now: Time,
        id: IntersectionID,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        self.wakeup_waiting(now, id, scheduler, map);
        let (_, _, remaining) = map
            .get_traffic_signal(id)
            .current_phase_and_remaining_time(now);
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
        now: Time,
        map: &Map,
        scheduler: &mut Scheduler,
        maybe_cars_and_queues: Option<(
            &Car,
            &BTreeMap<CarID, Car>,
            &mut BTreeMap<Traversable, Queue>,
        )>,
    ) -> bool {
        let req = Request { agent, turn };
        self.state
            .get_mut(&turn.parent)
            .unwrap()
            .waiting
            .entry(req.clone())
            .or_insert(now);

        let readonly_pair = maybe_cars_and_queues.as_ref().map(|(_, c, q)| (*c, &**q));
        let allowed = if self.use_freeform_policy_everywhere {
            self.freeform_policy(&req, map, readonly_pair)
        } else if let Some(ref signal) = map.maybe_get_traffic_signal(turn.parent) {
            self.traffic_signal_policy(&req, map, signal, speed, now, scheduler, readonly_pair)
        } else if let Some(ref sign) = map.maybe_get_stop_sign(turn.parent) {
            self.stop_sign_policy(&req, map, sign, now, scheduler, readonly_pair)
        } else {
            unreachable!()
        };
        if !allowed {
            return false;
        }

        // Don't block the box
        if let Some((car, _, queues)) = maybe_cars_and_queues {
            assert_eq!(agent, AgentID::Car(car.vehicle.id));
            let queue = queues.get_mut(&Traversable::Lane(turn.dst)).unwrap();
            if !queue.try_to_reserve_entry(
                car,
                !self.dont_block_the_box
                    || allow_block_the_box(map.get_i(turn.parent).orig_id.osm_node_id),
            ) {
                if self.break_turn_conflict_cycles {
                    // TODO Should we run the detector here?
                    if let Some(c) = queue.laggy_head {
                        self.blocked_by.insert((car.vehicle.id, c));
                    } else if let Some(c) = queue.cars.get(0) {
                        self.blocked_by.insert((car.vehicle.id, *c));
                    } else {
                        // Nobody's in the target lane, but there's somebody already in the
                        // intersection headed there, taking up all of the space.
                        self.blocked_by.insert((
                            car.vehicle.id,
                            self.state[&turn.parent]
                                .accepted
                                .iter()
                                .find(|r| r.turn.dst == turn.dst)
                                .unwrap()
                                .agent
                                .as_car(),
                        ));
                    }
                }

                return false;
            }
        }

        // TODO For now, we're only interested in signals, and there's too much raw data to store
        // for stop signs too.
        let state = self.state.get_mut(&turn.parent).unwrap();
        let delay = now - state.waiting.remove(&req).unwrap();
        if map.maybe_get_traffic_signal(state.id).is_some() {
            self.events
                .push(Event::IntersectionDelayMeasured(turn.parent, delay));
        }
        state.accepted.insert(req);
        if self.break_turn_conflict_cycles {
            if let AgentID::Car(car) = agent {
                retain_btreeset(&mut self.blocked_by, |(c, _)| *c != car);
            }
        }

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

    pub fn get_blocked_by(&self, a: AgentID) -> HashSet<AgentID> {
        let mut blocked_by = HashSet::new();
        if let AgentID::Car(c) = a {
            for (c1, c2) in &self.blocked_by {
                if *c1 == c {
                    blocked_by.insert(AgentID::Car(*c2));
                }
            }
        }
        blocked_by
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    pub fn delayed_intersections(
        &self,
        now: Time,
        threshold: Duration,
    ) -> Vec<(IntersectionID, Time)> {
        let mut candidates = Vec::new();
        for state in self.state.values() {
            if let Some(earliest) = state.waiting.values().min() {
                if now - *earliest >= threshold {
                    candidates.push((state.id, *earliest));
                }
            }
        }
        candidates.sort_by_key(|(_, t)| *t);
        candidates
    }

    // Weird way to measure this, but it works.
    pub fn worst_delay(
        &self,
        now: Time,
        map: &Map,
    ) -> (
        BTreeMap<RoadID, Duration>,
        BTreeMap<IntersectionID, Duration>,
    ) {
        let mut per_road = BTreeMap::new();
        let mut per_intersection = BTreeMap::new();
        for (i, state) in &self.state {
            for (req, t) in &state.waiting {
                {
                    let r = map.get_l(req.turn.src).parent;
                    let worst = per_road
                        .get(&r)
                        .cloned()
                        .unwrap_or(Duration::ZERO)
                        .max(now - *t);
                    per_road.insert(r, worst);
                }
                {
                    let worst = per_intersection
                        .get(i)
                        .cloned()
                        .unwrap_or(Duration::ZERO)
                        .max(now - *t);
                    per_intersection.insert(*i, worst);
                }
            }
        }
        (per_road, per_intersection)
    }
}

impl IntersectionSimState {
    fn freeform_policy(
        &mut self,
        req: &Request,
        map: &Map,
        maybe_cars_and_queues: Option<(&BTreeMap<CarID, Car>, &BTreeMap<Traversable, Queue>)>,
    ) -> bool {
        // Allow concurrent turns that don't conflict
        self.handle_accepted_conflicts(req, map, maybe_cars_and_queues)
    }

    fn stop_sign_policy(
        &mut self,
        req: &Request,
        map: &Map,
        sign: &ControlStopSign,
        now: Time,
        scheduler: &mut Scheduler,
        maybe_cars_and_queues: Option<(&BTreeMap<CarID, Car>, &BTreeMap<Traversable, Queue>)>,
    ) -> bool {
        if !self.handle_accepted_conflicts(req, map, maybe_cars_and_queues) {
            return false;
        }

        let our_priority = sign.get_priority(req.turn, map);
        assert!(our_priority != TurnPriority::Banned);
        let our_time = self.state[&req.turn.parent].waiting[req];

        if our_priority == TurnPriority::Yield && now < our_time + WAIT_AT_STOP_SIGN {
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
        &mut self,
        req: &Request,
        map: &Map,
        signal: &ControlTrafficSignal,
        speed: Speed,
        now: Time,
        scheduler: &mut Scheduler,
        maybe_cars_and_queues: Option<(&BTreeMap<CarID, Car>, &BTreeMap<Traversable, Queue>)>,
    ) -> bool {
        let turn = map.get_t(req.turn);

        // SharedSidewalkCorner doesn't conflict with anything -- fastpath!
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            return true;
        }

        let (_, phase, remaining_phase_time) = signal.current_phase_and_remaining_time(now);

        // Can't go at all this phase.
        let our_priority = phase.get_priority_of_turn(req.turn, signal);
        if our_priority == TurnPriority::Banned {
            return false;
        }

        // Somebody might already be doing a Yield turn that conflicts with this one.
        if !self.handle_accepted_conflicts(req, map, maybe_cars_and_queues) {
            return false;
        }

        let our_time = self.state[&req.turn.parent].waiting[req];
        if our_priority == TurnPriority::Yield
            && now < our_time + WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL
        {
            // Since we have "ownership" of scheduling for req.agent, don't need to use
            // scheduler.update.
            scheduler.push(
                our_time + WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL,
                Command::update_agent(req.agent),
            );
            return false;
        }

        // Previously: A yield loses to a conflicting Priority turn.
        // But similar to the description in stop_sign_policy, this caused unnecessary gridlock.
        // Priority vehicles getting scheduled first just requires a little tweak in
        // update_intersection.

        // TODO Make sure we can optimistically finish this turn before an approaching
        // higher-priority vehicle wants to begin.

        // Optimistically if nobody else is in the way, this is how long it'll take to finish the
        // turn. Don't start the turn if we won't finish by the time the light changes. If we get
        // it wrong, that's fine -- block the box a bit.
        let time_to_cross = turn.geom.length() / speed;
        if time_to_cross > remaining_phase_time {
            // Actually, we might have bigger problems...
            if time_to_cross > phase.duration {
                self.events.push(Event::Alert(
                    AlertLocation::Intersection(req.turn.parent),
                    format!(
                        "{:?} is impossible to fit into phase duration of {}",
                        req, phase.duration
                    ),
                ));
            } else {
                return false;
            }
        }

        true
    }

    // If true, the request can go.
    fn handle_accepted_conflicts(
        &mut self,
        req: &Request,
        map: &Map,
        maybe_cars_and_queues: Option<(&BTreeMap<CarID, Car>, &BTreeMap<Traversable, Queue>)>,
    ) -> bool {
        let turn = map.get_t(req.turn);
        let mut cycle_detected = false;
        let mut ok = true;
        for other in &self.state[&req.turn.parent].accepted {
            // Never short-circuit; always record all of the dependencies; it might help someone
            // else unstick things.
            if map.get_t(other.turn).conflicts_with(turn) {
                if self.break_turn_conflict_cycles {
                    if let AgentID::Car(c) = req.agent {
                        if let AgentID::Car(c2) = other.agent {
                            self.blocked_by.insert((c, c2));
                        }
                        if !cycle_detected {
                            if let Some(cycle) =
                                self.detect_conflict_cycle(c, maybe_cars_and_queues.unwrap())
                            {
                                // Allow the conflicting turn!
                                self.events.push(Event::Alert(
                                    AlertLocation::Intersection(req.turn.parent),
                                    format!("Turn conflict cycle involving {:?}", cycle),
                                ));
                                cycle_detected = true;
                            }
                        }
                    }
                }

                if !cycle_detected
                    && !allow_conflicting_turns(map.get_i(req.turn.parent).orig_id.osm_node_id)
                {
                    ok = false;
                }

                // It's never safe for two vehicles to go for the same lane.
                if turn.id.dst == other.turn.dst {
                    return false;
                }
            }
        }
        ok
    }

    fn detect_conflict_cycle(
        &self,
        car: CarID,
        pair: (&BTreeMap<CarID, Car>, &BTreeMap<Traversable, Queue>),
    ) -> Option<HashSet<CarID>> {
        let (cars, queues) = pair;

        let mut queue = vec![car];
        let mut seen = HashSet::new();
        while !queue.is_empty() {
            let current = queue.pop().unwrap();
            // Might not actually be a cycle. Insist on seeing the original req.agent
            // again.
            if !seen.is_empty() && current == car {
                return Some(seen);
            }
            if !seen.contains(&current) {
                seen.insert(current);

                for (c1, c2) in &self.blocked_by {
                    if *c1 == current {
                        queue.push(*c2);
                    }
                }

                // If this car isn't the head of its queue, add that dependency. (Except for
                // the original car, which we already know is the head of its queue)
                // TODO Maybe store this in blocked_by?
                if current != car {
                    let q = &queues[&cars[&current].router.head()];
                    let head = if let Some(c) = q.laggy_head {
                        c
                    } else {
                        *q.cars.get(0).unwrap()
                    };
                    if current != head {
                        queue.push(head);
                    }
                }
            }
        }
        None
    }
}

// TODO Sometimes a traffic signal is surrounded by tiny lanes with almost no capacity. Workaround
// for now.
fn allow_block_the_box(osm_node_id: i64) -> bool {
    // 23rd and Madison
    osm_node_id == 53211694 || osm_node_id == 53211693 ||
    // 31st and S Jackson
    osm_node_id == 53045512 ||
    // 23rd and Rainier
    osm_node_id == 53212741 || osm_node_id == 3496255252 ||
    // Rainier and Dearborn
    osm_node_id == 4607162574 ||
    // WA 509 and WA 99
    osm_node_id == 31327525 ||
    // 45th and Union Bay
    osm_node_id == 31192107 || osm_node_id == 4272330879 || osm_node_id == 53120147 ||
    // 45th and Montlake
    osm_node_id ==  31430639 || osm_node_id == 29977895 || osm_node_id == 3391705317 ||
    osm_node_id == 29977897 ||
    // Rainier and Brandon
    osm_node_id == 53089293 ||
    // Rainier and Letitia
    osm_node_id == 1729797719 || osm_node_id == 4272388873 || osm_node_id == 53194882 ||
    // Rainier and MLK
    osm_node_id == 53131428 ||
    // Montlake and 520
    osm_node_id == 53128053
}

// TODO Various problems (bad geometry, multi-intersection turn restrictions) cause
// vehicles to unrealistically block each other.
#[rustfmt::skip]
fn allow_conflicting_turns(osm_node_id: i64) -> bool {
    vec![
        // Montlake and 520
        29449863, 29464223, 3391701882, 3391701883,
        // Boyer and Lynn
        3978753095,
        // WA 509 and WA 99
        31253092,
        // 52nd and Holly
        4263867891, 4263867898, 4263867908, 4263867899
    ]
    .contains(&osm_node_id)
}
