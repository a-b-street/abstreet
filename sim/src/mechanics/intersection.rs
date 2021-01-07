use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use abstutil::{
    deserialize_btreemap, prettyprint_usize, retain_btreeset, serialize_btreemap, FixedMap,
};
use geom::{Duration, Time};
use map_model::{
    ControlStopSign, ControlTrafficSignal, Intersection, IntersectionID, LaneID, Map, StageType,
    Traversable, TurnID, TurnPriority, TurnType, UberTurn,
};

use crate::mechanics::car::Car;
use crate::mechanics::Queue;
use crate::{
    AgentID, AlertLocation, CarID, Command, DelayCause, Event, Scheduler, SimOptions, Speed,
};

const WAIT_AT_STOP_SIGN: Duration = Duration::const_seconds(0.5);
const WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL: Duration = Duration::const_seconds(0.2);

/// Manages conflicts at intersections. When an agent has reached the end of a lane, they call
/// maybe_start_turn to make a Request. Based on the intersection type (stop sign, traffic signal,
/// or a "freeform policy"), the Request gets queued or immediately accepted. When agents finish
/// turns or when some time passes (for traffic signals), the intersection also gets a chance to
/// react, maybe granting one of the pending requests.
///
/// Most of the complexity comes from attempting to workaround
/// <https://dabreegster.github.io/abstreet/trafficsim/gridlock.html>.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct IntersectionSimState {
    state: BTreeMap<IntersectionID, State>,
    use_freeform_policy_everywhere: bool,
    dont_block_the_box: bool,
    break_turn_conflict_cycles: bool,
    handle_uber_turns: bool,
    disable_turn_conflicts: bool,
    // (x, y) means x is blocked by y. It's a many-to-many relationship. TODO Better data
    // structure.
    blocked_by: BTreeSet<(CarID, CarID)>,
    events: Vec<Event>,

    // Count how many calls to maybe_start_turn there are aside from the initial call. Break down
    // failures by those not allowed by the current intersection state vs those blocked by a
    // vehicle in the way in the target queue.
    total_repeat_requests: usize,
    not_allowed_requests: usize,
    blocked_by_someone_requests: usize,
}

#[derive(Clone, Serialize, Deserialize)]
struct State {
    id: IntersectionID,
    // The in-progress turns which any potential new turns must not conflict with
    accepted: BTreeSet<Request>,
    // Track when a request is first made.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    waiting: BTreeMap<Request, Time>,
    // When a vehicle begins an uber-turn, reserve the future turns to ensure they're able to
    // complete the entire sequence. This is especially necessary since groups of traffic signals
    // are not yet configured as one.
    reserved: BTreeSet<Request>,

    signal: Option<SignalState>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SignalState {
    // The current stage of the signal, zero based
    current_stage: usize,
    // The time when the signal is checked for advancing
    stage_ends_at: Time,
    // The number of times a variable signal has been extended during the current stage.
    extensions_count: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone, Debug)]
struct Request {
    agent: AgentID,
    turn: TurnID,
}

// Mutations
impl IntersectionSimState {
    pub fn new(map: &Map, scheduler: &mut Scheduler, opts: &SimOptions) -> IntersectionSimState {
        let mut sim = IntersectionSimState {
            state: BTreeMap::new(),
            use_freeform_policy_everywhere: opts.use_freeform_policy_everywhere,
            dont_block_the_box: opts.dont_block_the_box,
            break_turn_conflict_cycles: opts.break_turn_conflict_cycles,
            handle_uber_turns: opts.handle_uber_turns,
            disable_turn_conflicts: opts.disable_turn_conflicts,
            blocked_by: BTreeSet::new(),
            events: Vec::new(),

            total_repeat_requests: 0,
            not_allowed_requests: 0,
            blocked_by_someone_requests: 0,
        };
        if sim.disable_turn_conflicts {
            sim.use_freeform_policy_everywhere = true;
        }

        for i in map.all_intersections() {
            let mut state = State {
                id: i.id,
                accepted: BTreeSet::new(),
                waiting: BTreeMap::new(),
                reserved: BTreeSet::new(),
                signal: None,
            };
            if i.is_traffic_signal() {
                state.signal = Some(SignalState::new(i.id, Time::START_OF_DAY, map, scheduler));
            }
            sim.state.insert(i.id, state);
        }
        sim
    }

    pub fn turn_finished(
        &mut self,
        now: Time,
        agent: AgentID,
        turn: TurnID,
        scheduler: &mut Scheduler,
        map: &Map,
        handling_live_edits: bool,
    ) {
        let state = self.state.get_mut(&turn.parent).unwrap();
        assert!(state.accepted.remove(&Request { agent, turn }));
        state.reserved.remove(&Request { agent, turn });
        if !handling_live_edits && map.get_t(turn).turn_type != TurnType::SharedSidewalkCorner {
            self.wakeup_waiting(now, turn.parent, scheduler, map);
        }
        if self.break_turn_conflict_cycles {
            if let AgentID::Car(car) = agent {
                // todo: when drain_filter() is no longer experimental, use it instead of
                // retian_btreeset()
                retain_btreeset(&mut self.blocked_by, |(_, c)| *c != car);
            }
        }
    }

    /// For deleting cars
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

    /// Vanished at border, stopped biking, etc -- a vehicle disappeared, and didn't have one last
    /// turn.
    pub fn vehicle_gone(&mut self, car: CarID) {
        retain_btreeset(&mut self.blocked_by, |(c1, c2)| *c1 != car && *c2 != car);
    }

    pub fn agent_deleted_mid_turn(&mut self, agent: AgentID, turn: TurnID) {
        let state = self.state.get_mut(&turn.parent).unwrap();
        assert!(state.accepted.remove(&Request { agent, turn }));
        state.reserved.remove(&Request { agent, turn });
    }

    fn wakeup_waiting(&self, now: Time, i: IntersectionID, scheduler: &mut Scheduler, map: &Map) {
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
            let stage = &signal.stages[self.state[&i].signal.as_ref().unwrap().current_stage];
            for (req, _) in all {
                match stage.get_priority_of_turn(req.turn, signal) {
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
                match sign.get_priority(req.turn, map) {
                    TurnPriority::Protected => {
                        protected.push(req);
                    }
                    TurnPriority::Yield => {
                        yielding.push(req);
                    }
                    TurnPriority::Banned => unreachable!(),
                }
            }
        } else {
            // This could either be a border intersection or an intersection that was just closed
            // in the middle of simulation. In either case, there shouldn't be any other turns at
            // it.
            assert!(protected.is_empty());
            assert!(yielding.is_empty());
        };

        for req in protected {
            // Use update because multiple agents could finish a turn at the same time, before the
            // waiting one has a chance to try again.
            scheduler.update(now, Command::update_agent(req.agent));
        }
        // Make sure the protected movement gets first dibs. The scheduler arbitrarily (but
        // deterministically) orders commands with the same time.
        for req in yielding {
            scheduler.update(
                now + Duration::seconds(0.1),
                Command::update_agent(req.agent),
            );
        }
    }

    /// This is only triggered for traffic signals.
    pub fn update_intersection(
        &mut self,
        now: Time,
        id: IntersectionID,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        // trivial function that advances the signal stage and returns duration
        fn advance(signal_state: &mut SignalState, signal: &ControlTrafficSignal) -> Duration {
            signal_state.current_stage = (signal_state.current_stage + 1) % signal.stages.len();
            signal.stages[signal_state.current_stage]
                .stage_type
                .simple_duration()
        }

        let state = self.state.get_mut(&id).unwrap();
        let signal_state = state.signal.as_mut().unwrap();
        let signal = map.get_traffic_signal(id);
        let duration: Duration;
        // Switch to a new stage?
        assert_eq!(now, signal_state.stage_ends_at);
        let old_stage = &signal.stages[signal_state.current_stage];
        match old_stage.stage_type {
            StageType::Fixed(_) => {
                duration = advance(signal_state, signal);
            }
            StageType::Variable(min, delay, additional) => {
                // test if anyone is waiting in current stage, and if so, extend the signal cycle.
                // Filter out pedestrians, as they've had their chance and the delay
                // could be short enough to keep them on the curb.
                let delay = std::cmp::max(Duration::const_seconds(1.0), delay);
                // Only extend for the fixed additional time
                if signal_state.extensions_count as f64 * delay.inner_seconds()
                    >= additional.inner_seconds()
                {
                    self.events.push(Event::Alert(
                        AlertLocation::Intersection(id),
                        format!(
                            "exhausted a variable stage {},{},{},{}",
                            min, delay, additional, signal_state.extensions_count
                        ),
                    ));
                    duration = advance(signal_state, signal);
                    signal_state.extensions_count = 0;
                } else if state.waiting.keys().all(|req| {
                    if let AgentID::Pedestrian(_) = req.agent {
                        return true;
                    }
                    // Should we only allow protected to extend or any not banned?
                    // currently only the protected demand control extended.
                    old_stage.get_priority_of_turn(req.turn, signal) != TurnPriority::Protected
                }) {
                    signal_state.extensions_count = 0;
                    duration = advance(signal_state, signal);
                } else {
                    signal_state.extensions_count += 1;
                    duration = delay;
                    self.events.push(Event::Alert(
                        AlertLocation::Intersection(id),
                        format!(
                            "Extending a variable stage {},{},{},{}",
                            min, delay, additional, signal_state.extensions_count
                        ),
                    ));
                }
            }
        }

        signal_state.stage_ends_at = now + duration;
        scheduler.push(signal_state.stage_ends_at, Command::UpdateIntersection(id));
        self.wakeup_waiting(now, id, scheduler, map);
    }

    /// For cars: The head car calls this when they're at the end of the lane WaitingToAdvance. If
    /// this returns true, then the head car MUST actually start this turn.
    /// For peds: Likewise -- only called when the ped is at the start of the turn. They must
    /// actually do the turn if this returns true.
    ///
    /// If this returns false, the agent should NOT retry. IntersectionSimState will schedule a
    /// retry event at some point.
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
            &FixedMap<CarID, Car>,
            &mut HashMap<Traversable, Queue>,
        )>,
    ) -> bool {
        let req = Request { agent, turn };
        let entry = self
            .state
            .get_mut(&turn.parent)
            .unwrap()
            .waiting
            .entry(req.clone());
        let repeat_request = match entry {
            std::collections::btree_map::Entry::Vacant(_) => false,
            std::collections::btree_map::Entry::Occupied(_) => true,
        };
        entry.or_insert(now);

        if repeat_request {
            self.total_repeat_requests += 1;
        }

        let shared_sidewalk_corner =
            map.get_t(req.turn).turn_type == TurnType::SharedSidewalkCorner;

        let readonly_pair = maybe_cars_and_queues.as_ref().map(|(_, c, q)| (*c, &**q));
        let allowed = if shared_sidewalk_corner {
            // SharedSidewalkCorner doesn't conflict with anything -- fastpath!
            true
        } else if !self.handle_accepted_conflicts(&req, map, readonly_pair) {
            // It's never OK to perform a conflicting turn
            false
        } else if maybe_cars_and_queues
            .as_ref()
            .map(|(car, _, _)| {
                self.handle_uber_turns && car.router.get_path().currently_inside_ut().is_some()
            })
            .unwrap_or(false)
        {
            // If we started an uber-turn, then finish it! But alert if we're running a red light.
            if let Some(ref signal) = map.maybe_get_traffic_signal(turn.parent) {
                // Don't pass in the scheduler, aka, don't pause before yielding.
                if !self.traffic_signal_policy(&req, map, signal, speed, now, None) && false {
                    self.events.push(Event::Alert(
                        AlertLocation::Intersection(req.turn.parent),
                        format!("Running a red light inside an uber-turn: {:?}", req),
                    ));
                }
            }

            true
        } else if self.use_freeform_policy_everywhere {
            // If we made it this far, we don't conflict with an accepted turn
            true
        } else if let Some(ref signal) = map.maybe_get_traffic_signal(turn.parent) {
            self.traffic_signal_policy(&req, map, signal, speed, now, Some(scheduler))
        } else if let Some(ref sign) = map.maybe_get_stop_sign(turn.parent) {
            self.stop_sign_policy(&req, map, sign, now, scheduler)
        } else {
            unreachable!()
        };
        if !allowed {
            if repeat_request {
                self.not_allowed_requests += 1;
            }
            return false;
        }

        // Lock the entire uber-turn.
        if self.handle_uber_turns {
            if let Some(ut) = maybe_cars_and_queues
                .as_ref()
                .and_then(|(car, _, _)| car.router.get_path().about_to_start_ut())
            {
                // If there's a problem up ahead, don't start.
                for t in &ut.path {
                    let req = Request { agent, turn: *t };
                    if !self.handle_accepted_conflicts(&req, map, readonly_pair) {
                        if repeat_request {
                            self.blocked_by_someone_requests += 1;
                        }
                        return false;
                    }
                }
                // If the way is clear, make sure it stays that way.
                for t in &ut.path {
                    self.state
                        .get_mut(&t.parent)
                        .unwrap()
                        .reserved
                        .insert(Request { agent, turn: *t });
                }
            }
        }

        // Don't block the box.
        if let Some((car, _, queues)) = maybe_cars_and_queues {
            assert_eq!(agent, AgentID::Car(car.vehicle.id));
            let inside_ut = self.handle_uber_turns
                && (car.router.get_path().currently_inside_ut().is_some()
                    || car.router.get_path().about_to_start_ut().is_some());
            let queue = queues.get_mut(&Traversable::Lane(turn.dst)).unwrap();
            if !queue.try_to_reserve_entry(
                car,
                !self.dont_block_the_box
                    || allow_block_the_box(map.get_i(turn.parent))
                    || inside_ut,
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
                        // I guess we shouldn't count reservations for uber-turns here, because
                        // we're not going to do block-the-box resolution in the interior at
                        // all?
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

                if repeat_request {
                    self.blocked_by_someone_requests += 1;
                }
                return false;
            }
        }

        // TODO For now, we're only interested in signals, and there's too much raw data to store
        // for stop signs too.
        let state = self.state.get_mut(&turn.parent).unwrap();
        let delay = now - state.waiting.remove(&req).unwrap();
        // SharedSidewalkCorner are always no-conflict, immediate turns; they're not interesting.
        if !shared_sidewalk_corner {
            if let Some(ts) = map.maybe_get_traffic_signal(state.id) {
                self.events.push(Event::IntersectionDelayMeasured(
                    ts.compressed_id(turn),
                    delay,
                    agent,
                ));
            }
        }
        state.accepted.insert(req);
        if self.break_turn_conflict_cycles {
            if let AgentID::Car(car) = agent {
                retain_btreeset(&mut self.blocked_by, |(c, _)| *c != car);
            }
        }

        true
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    pub fn handle_live_edited_traffic_signals(
        &mut self,
        now: Time,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        for state in self.state.values_mut() {
            match (
                map.maybe_get_traffic_signal(state.id),
                state.signal.as_mut(),
            ) {
                (Some(ts), Some(signal_state)) => {
                    if signal_state.current_stage >= ts.stages.len() {
                        // Just jump back to the first one. Shrug.
                        signal_state.current_stage = 0;
                        println!(
                            "WARNING: Traffic signal {} was live-edited in the middle of a stage, \
                             so jumping back to the first stage",
                            state.id
                        );
                    }
                }
                (Some(_), None) => {
                    state.signal = Some(SignalState::new(state.id, now, map, scheduler));
                }
                (None, Some(_)) => {
                    state.signal = None;
                    scheduler.cancel(Command::UpdateIntersection(state.id));
                }
                (None, None) => {}
            }
        }
    }

    pub fn handle_live_edits(&self, map: &Map) {
        // Just sanity check that we don't have any references to deleted turns
        let mut errors = Vec::new();
        for state in self.state.values() {
            for req in &state.accepted {
                if map.maybe_get_t(req.turn).is_none() {
                    errors.push(format!("{} accepted for {}", req.agent, req.turn));
                }
            }
            for req in state.waiting.keys() {
                if map.maybe_get_t(req.turn).is_none() {
                    errors.push(format!("{} waiting for {}", req.agent, req.turn));
                }
            }
            for req in &state.reserved {
                if map.maybe_get_t(req.turn).is_none() {
                    errors.push(format!("{} has reserved {}", req.agent, req.turn));
                }
            }
        }
        if !errors.is_empty() {
            for x in errors {
                error!("{}", x);
            }
            panic!("After live map edits, intersection state refers to deleted turns!");
        }
    }
}

// Queries
impl IntersectionSimState {
    pub fn nobody_headed_towards(&self, lane: LaneID, i: IntersectionID) -> bool {
        let state = &self.state[&i];
        !state
            .accepted
            .iter()
            .chain(state.reserved.iter())
            .any(|req| req.turn.dst == lane)
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

    pub fn get_accepted_agents(&self, id: IntersectionID) -> Vec<(AgentID, TurnID)> {
        self.state[&id]
            .accepted
            .iter()
            .map(|req| (req.agent, req.turn))
            .collect()
    }

    pub fn get_waiting_agents(&self, id: IntersectionID) -> Vec<(AgentID, TurnID, Time)> {
        self.state[&id]
            .waiting
            .iter()
            .map(|(req, time)| (req.agent, req.turn, *time))
            .collect()
    }

    /// Returns intersections with travelers waiting for at least `threshold` since `now`, ordered
    /// so the longest delayed intersection is first.
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

    pub fn current_stage_and_remaining_time(
        &self,
        now: Time,
        i: IntersectionID,
    ) -> (usize, Duration) {
        let state = &self.state[&i].signal.as_ref().unwrap();
        if now > state.stage_ends_at {
            panic!(
                "At {}, but {} should have advanced its stage at {}",
                now, i, state.stage_ends_at
            );
        }
        (state.current_stage, state.stage_ends_at - now)
    }

    pub fn describe_stats(&self) -> Vec<String> {
        vec![
            format!("intersection stats"),
            format!(
                "{} total turn requests repeated after the initial attempt",
                prettyprint_usize(self.total_repeat_requests)
            ),
            format!(
                "{} not allowed by intersection ({}%)",
                prettyprint_usize(self.not_allowed_requests),
                (100.0 * (self.not_allowed_requests as f64) / (self.total_repeat_requests as f64))
                    .round()
            ),
            format!(
                "{} blocked by someone in the way ({}%)",
                prettyprint_usize(self.blocked_by_someone_requests),
                (100.0 * (self.blocked_by_someone_requests as f64)
                    / (self.total_repeat_requests as f64))
                    .round()
            ),
        ]
    }

    pub fn populate_blocked_by(
        &self,
        now: Time,
        graph: &mut BTreeMap<AgentID, (Duration, DelayCause)>,
        map: &Map,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) {
        // Don't use self.blocked_by -- that gets complicated with uber-turns and such.
        //
        // This also assumes default values for handle_uber_turns, disable_turn_conflicts, etc!
        for state in self.state.values() {
            for (req, started_at) in &state.waiting {
                let turn = map.get_t(req.turn);
                // In the absence of other explanations, the agent must be pausing at a stop sign
                // or before making an unprotected movement, aka, in the middle of
                // WAIT_AT_STOP_SIGN or WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL. Or they're waiting for
                // a signal to change.
                let mut cause = DelayCause::Intersection(state.id);
                if let Some(other) = state.accepted.iter().find(|other| {
                    turn.conflicts_with(map.get_t(other.turn)) || turn.id == other.turn
                }) {
                    cause = DelayCause::Agent(other.agent);
                } else if let AgentID::Car(car) = req.agent {
                    let queue = &queues[&Traversable::Lane(req.turn.dst)];
                    let car = cars.get(&car).unwrap();
                    if !queue.room_for_car(car) {
                        // TODO Or it's reserved due to an uber turn or something
                        let blocker = queue.cars.back().cloned().or(queue.laggy_head).unwrap();
                        cause = DelayCause::Agent(AgentID::Car(blocker));
                    } else if let Some(ut) = car.router.get_path().about_to_start_ut() {
                        if let Some(blocker) = self.check_for_conflicts_before_uber_turn(ut, map) {
                            cause = DelayCause::Agent(blocker);
                        }
                    }
                }
                graph.insert(req.agent, (now - *started_at, cause));
            }
        }
    }

    /// See if any agent is currently performing a turn that conflicts with an uber-turn. Doesn't
    /// check for room on the queues.
    fn check_for_conflicts_before_uber_turn(&self, ut: &UberTurn, map: &Map) -> Option<AgentID> {
        for t in &ut.path {
            let turn = map.get_t(*t);
            let state = &self.state[&turn.id.parent];
            for other in state.accepted.iter().chain(state.reserved.iter()) {
                if map.get_t(other.turn).conflicts_with(turn) {
                    return Some(other.agent);
                }
            }
        }
        None
    }
}

// Stuff to support maybe_start_turn
impl IntersectionSimState {
    fn stop_sign_policy(
        &mut self,
        req: &Request,
        map: &Map,
        sign: &ControlStopSign,
        now: Time,
        scheduler: &mut Scheduler,
    ) -> bool {
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
        scheduler: Option<&mut Scheduler>,
    ) -> bool {
        let turn = map.get_t(req.turn);

        let state = &self.state[&req.turn.parent];
        let signal_state = state.signal.as_ref().unwrap();
        let stage = &signal.stages[signal_state.current_stage];
        let full_stage_duration = stage.stage_type.simple_duration();
        let remaining_stage_time = signal_state.stage_ends_at - now;
        let our_time = state.waiting[req];

        // Can't go at all this stage.
        let our_priority = stage.get_priority_of_turn(req.turn, signal);
        if our_priority == TurnPriority::Banned {
            return false;
        }

        if our_priority == TurnPriority::Yield
            && now < our_time + WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL
        {
            // Since we have "ownership" of scheduling for req.agent, don't need to use
            // scheduler.update.
            if let Some(s) = scheduler {
                s.push(
                    our_time + WAIT_BEFORE_YIELD_AT_TRAFFIC_SIGNAL,
                    Command::update_agent(req.agent),
                );
            }
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
        if time_to_cross > remaining_stage_time {
            // Actually, we might have bigger problems...
            if time_to_cross > full_stage_duration {
                self.events.push(Event::Alert(
                    AlertLocation::Intersection(req.turn.parent),
                    format!(
                        "{:?} is impossible to fit into stage duration of {}",
                        req, full_stage_duration
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
        maybe_cars_and_queues: Option<(&FixedMap<CarID, Car>, &HashMap<Traversable, Queue>)>,
    ) -> bool {
        let turn = map.get_t(req.turn);
        let mut cycle_detected = false;
        let mut ok = true;
        for other in self.state[&req.turn.parent]
            .accepted
            .iter()
            .chain(self.state[&req.turn.parent].reserved.iter())
        {
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

                if !cycle_detected && !self.disable_turn_conflicts {
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
        pair: (&FixedMap<CarID, Car>, &HashMap<Traversable, Queue>),
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

impl SignalState {
    fn new(id: IntersectionID, now: Time, map: &Map, scheduler: &mut Scheduler) -> SignalState {
        let mut state = SignalState {
            current_stage: 0,
            stage_ends_at: now,
            extensions_count: 0,
        };

        let signal = map.get_traffic_signal(id);
        // What stage are we starting with?
        let mut offset = (now - Time::START_OF_DAY) + signal.offset;
        loop {
            let dt = signal.stages[state.current_stage]
                .stage_type
                .simple_duration();
            if offset >= dt {
                offset -= dt;
                state.current_stage += 1;
                if state.current_stage == signal.stages.len() {
                    state.current_stage = 0;
                }
            } else {
                state.stage_ends_at = now + dt - offset;
                break;
            }
        }
        scheduler.push(state.stage_ends_at, Command::UpdateIntersection(id));
        state
    }
}

fn allow_block_the_box(i: &Intersection) -> bool {
    // Degenerate intersections are often just artifacts of how roads are split up in OSM. Allow
    // vehicles to get stuck in them, since the only possible thing they could block is pedestrians
    // from using the crosswalk. Those crosswalks usually don't exist in reality, so this behavior
    // is more realistic.
    if i.roads.len() == 2 {
        return true;
    }

    // TODO Sometimes a traffic signal is surrounded by tiny lanes with almost no capacity.
    // Workaround for now.
    //
    // When adding new cases:
    // 1) Organize by which map the intersection fixes
    // 2) Ensure a prebaked scenario covers this, to track regressions and make sure it actually
    //    helps.
    let id = i.orig_id.0;
    // lakeslice
    if id == 53211693 || id == 53214134 || id == 53214133 || id == 987334546 || id == 848817336 {
        return true;
    }
    false
}
