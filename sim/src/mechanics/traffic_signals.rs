use crate::{Command, Scheduler};
use geom::{Duration, Time};
use map_model::{
    ControlTrafficSignal, IntersectionID, SignalTimerType, TrafficControlType, Turn, TurnGroupID,
    TurnID, TurnPriority, TurnType,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

type Call = (TurnGroupID, Time);

pub trait YellowChecker {
    fn is_turn_group_yellow(&self, id: &TurnGroupID) -> bool;
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct TrafficSignalState {
    pub id: IntersectionID,
    pub current_stage: usize,
    pub stage_ends_at: Time,
    pub green_must_end_at: Time,
    turn_group_state: BTreeMap<TurnGroupID, TurnGroupState>,
    phase_state: Vec<PhaseState>,
}

impl YellowChecker for TrafficSignalState {
    fn is_turn_group_yellow(&self, id: &TurnGroupID) -> bool {
        self.turn_group_state.get(id).unwrap().is_yellow
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct TurnGroupState {
    pub is_yellow: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct PhaseState {
    pub is_called: bool,
    pub last_called: Time,
}

impl PhaseState {
    pub fn new() -> PhaseState {
        PhaseState {
            is_called: false,
            last_called: Time::START_OF_DAY,
        }
    }

    pub fn clear(&mut self) {
        self.is_called = false;
        self.last_called = Time::START_OF_DAY;
    }
}

// Development note: I purposely left this as not a method of TrafficSignalState.
// If anything, the logical behavior of a traffic signal is more of an immutable
// property of `ControlTrafficSignal` on the map model, so it doesn't feel right
// to privilege the state as self.
pub fn update_traffic_signal(
    now: Time,
    intersection_id: IntersectionID,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    scheduler: &mut Scheduler,
) {
    match signal.control_type {
        TrafficControlType::Actuated => {
            update_actuated(now, intersection_id, state, signal, scheduler)
        }
        TrafficControlType::PreTimed => {
            update_pretimed(now, intersection_id, state, signal, scheduler)
        }
    }
}

fn update_pretimed(
    now: Time,
    intersection_id: IntersectionID,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    scheduler: &mut Scheduler,
) {
    if now >= state.stage_ends_at {
        state.set_green_all_current_stage(signal);

        state.current_stage = (state.current_stage + 1) % signal.stages.len();

        state.stage_ends_at = now
            + signal.stages[state.current_stage]
                .phase_type
                .simple_duration();

        scheduler.update(
            state.stage_ends_at - signal.yellow_duration,
            Command::UpdateIntersection(intersection_id, None, None),
        );
    } else if now >= state.stage_ends_at - signal.yellow_duration {
        state.set_yellow_all_current_stage(signal);

        scheduler.update(
            state.stage_ends_at,
            Command::UpdateIntersection(intersection_id, None, None),
        );
    } else {
        // Should only get here on the very first update for this signal.
        state.set_green_all_current_stage(signal);

        scheduler.update(
            state.stage_ends_at - signal.yellow_duration,
            Command::UpdateIntersection(intersection_id, None, None),
        );
    }
}

fn update_actuated(
    now: Time,
    intersection_id: IntersectionID,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    scheduler: &mut Scheduler,
) {
    state.stage_ends_at = now + Duration::hours(1);

    scheduler.update(
        state.stage_ends_at,
        Command::UpdateIntersection(intersection_id, None, None),
    );
}

pub fn actuate_traffic_signal(
    now: Time,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    turn: &Turn,
    scheduler: &mut Scheduler,
) {
    match signal.control_type {
        TrafficControlType::Actuated => actuate(now, state, signal, turn, scheduler),
        TrafficControlType::PreTimed => {} // Do nothing!
    };
}

fn actuate(
    now: Time,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    turn: &Turn,
    scheduler: &mut Scheduler,
) {
    if turn.turn_type == TurnType::SharedSidewalkCorner {
        return;
    }

    // TODO Change this to hinge on whether phase is active or not, not which stage it's in.

    // Find stage to actuate, if there is one.
    let maybe_stage_index = match maybe_get_protected_stage_index(turn.id, state, signal) {
        Some(stage) => Some(stage),
        None => maybe_get_yield_stage_index(turn.id, state, signal),
    };

    // Exit if there is no stage to actuate.
    if maybe_stage_index.is_none() {
        return;
    }

    // Call the stage.
    let i = maybe_stage_index.unwrap();
    let called_phase_state = &mut state.phase_state[i];
    called_phase_state.is_called = true;
    called_phase_state.last_called = now;

    // If caller is vehicle and called stage is current stage,
    // set a new timer for passage time "gap out", if needed.
    if !turn.between_sidewalks() && (state.current_stage == i) {
        let new_green_expiration = now + signal.stages[i].passage_time;

        if new_green_expiration < state.green_must_end_at {
            scheduler.update(
                new_green_expiration,
                Command::UpdateIntersection(
                    signal.id,
                    Some(signal.turn_to_group(turn.id)),
                    Some(SignalTimerType::PassageTimer),
                ),
            )
        }
    }

    // TODO Change to If called phase is not active.
    // If called stage is *not* active, activate MaxGreen timer
    // if not already activated.
    if state.current_stage != i {
        let new_green_expiration = now + signal.stages[i].maximum_green;

        if new_green_expiration < state.green_must_end_at {
            state.green_must_end_at = new_green_expiration;

            // Set MaxGreenTimer for protected turn groups.
            // Although the draw routine cares about color of yield turn groups, the control
            // does not. Not sure if control should support coloring for yield groups.
            // If yield groups have signal color in real life, it's a flashing yellow.
            let current_stage = &signal.stages[state.current_stage];
            for tg in current_stage.protected_groups_iter() {
                scheduler.update(
                    new_green_expiration,
                    Command::UpdateIntersection(
                        signal.id,
                        Some(*tg),
                        Some(SignalTimerType::MaxGreenTimer),
                    ),
                );
            }
        }
    }
}

fn maybe_get_protected_stage_index(
    turn: TurnID,
    state: &TrafficSignalState,
    signal: &ControlTrafficSignal,
) -> Option<usize> {
    // Find the soonest stage with the turn (which might be the current stage).
    // (In edge case where every stage has the turn, just call the current stage).

    // TODO: Build a mapping from turn group to stage in advance instead
    // of computing for every call?

    // See if there is at least one protected turn group with this turn.
    let turn_group_id = signal.turn_to_group(turn);

    let num_stages = signal.stages.len();

    let maybe_soonest_stage = signal
        .stages
        .iter()
        .cycle()
        .skip(state.current_stage)
        .take(num_stages)
        .enumerate()
        .find(|enumerated_stage| {
            TurnPriority::Protected == enumerated_stage.1.get_priority_of_group(turn_group_id)
        });

    if maybe_soonest_stage == None {
        return None;
    }

    let (num_after_current, _) = maybe_soonest_stage.unwrap();

    let soonest_stage_index = (state.current_stage + num_after_current) % num_stages;

    return Some(soonest_stage_index);
}

fn maybe_get_yield_stage_index(
    turn: TurnID,
    state: &TrafficSignalState,
    signal: &ControlTrafficSignal,
) -> Option<usize> {
    // See if there is at least one yield turn group with this turn.
    let turn_group_id = signal.turn_to_group(turn);

    let num_stages = signal.stages.len();

    let maybe_soonest_stage = signal
        .stages
        .iter()
        .cycle()
        .skip(state.current_stage)
        .take(num_stages)
        .enumerate()
        .find(|enumerated_stage| {
            TurnPriority::Yield == enumerated_stage.1.get_priority_of_group(turn_group_id)
        });

    if maybe_soonest_stage == None {
        return None;
    }

    let (num_after_current, _) = maybe_soonest_stage.unwrap();

    let soonest_stage_index = (state.current_stage + num_after_current) % num_stages;

    return Some(soonest_stage_index);
}

impl TrafficSignalState {
    pub fn new(signal: &ControlTrafficSignal) -> TrafficSignalState {
        let mut state = TrafficSignalState {
            id: IntersectionID(0),
            current_stage: 0,
            stage_ends_at: Time::START_OF_DAY,
            green_must_end_at: Time::START_OF_DAY + Duration::hours(1),
            turn_group_state: BTreeMap::<TurnGroupID, TurnGroupState>::new(),
            phase_state: Vec::<PhaseState>::new(),
        };

        state.initialize(signal);

        return state;
    }

    fn initialize(&mut self, signal: &ControlTrafficSignal) {
        self.id = signal.id;

        // What stage are we starting with?
        let mut offset = signal.offset;
        loop {
            let dt = signal.stages[self.current_stage]
                .phase_type
                .simple_duration();
            if offset >= dt {
                offset -= dt;
                self.current_stage += 1;
                if self.current_stage == signal.stages.len() {
                    self.current_stage = 0;
                }
            } else {
                self.stage_ends_at = Time::START_OF_DAY + dt - offset;
                break;
            }
        }

        // Initialize turn group state
        for stage in signal.stages.iter() {
            for turn_group_id in stage.protected_groups_iter() {
                self.turn_group_state
                    .insert(*turn_group_id, TurnGroupState { is_yellow: false });
            }
            for turn_group_id in stage.yield_groups_iter() {
                self.turn_group_state
                    .insert(*turn_group_id, TurnGroupState { is_yellow: false });
            }
            self.phase_state.push(PhaseState::new());
        }
    }

    fn set_green_all_current_stage(&mut self, signal: &ControlTrafficSignal) {
        let current_stage = &signal.stages[self.current_stage];

        for turn_group in current_stage.protected_groups_iter() {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = false;
        }

        for turn_group in current_stage.yield_groups_iter() {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = false;
        }
    }

    fn set_yellow_all_current_stage(&mut self, signal: &ControlTrafficSignal) {
        let current_stage = &signal.stages[self.current_stage];

        for turn_group in current_stage.protected_groups_iter() {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = true;
        }

        for turn_group in current_stage.yield_groups_iter() {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = true;
        }
    }
}
