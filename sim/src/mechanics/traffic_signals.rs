use crate::{Command, Scheduler};
use geom::{Duration, Time};
use map_model::{
    ControlTrafficSignal, IntersectionID, TrafficControlType, TurnGroupID, TurnID, TurnPriority,
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
    pub current_phase: usize,
    pub phase_ends_at: Time,
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
    if now >= state.phase_ends_at {
        state.set_green_all_current_phase(signal);

        state.current_phase = (state.current_phase + 1) % signal.phases.len();

        state.phase_ends_at = now
            + signal.phases[state.current_phase]
                .phase_type
                .simple_duration();

        scheduler.push(
            state.phase_ends_at - signal.yellow_duration,
            Command::UpdateIntersection(intersection_id),
        );
    } else if now >= state.phase_ends_at - signal.yellow_duration {
        state.set_yellow_all_current_phase(signal);

        scheduler.push(
            state.phase_ends_at,
            Command::UpdateIntersection(intersection_id),
        );
    } else {
        // Should only get here on the very first call for this signal.
        state.set_green_all_current_phase(signal);

        scheduler.push(
            state.phase_ends_at - signal.yellow_duration,
            Command::UpdateIntersection(intersection_id),
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
    state.phase_ends_at = now + Duration::hours(1);

    scheduler.push(
        state.phase_ends_at,
        Command::UpdateIntersection(intersection_id),
    );
}

pub fn actuate_traffic_signal(
    now: Time,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    turn: TurnID,
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
    turn: TurnID,
    scheduler: &mut Scheduler,
) {
    // Find phase to actuate, if there is one.
    let maybe_phase_index = match maybe_get_protected_phase_index(turn, state, signal) {
        Some(phase) => Some(phase),
        None => maybe_get_yield_phase_index(turn, state, signal),
    };

    // Exit if there is no phase to actuate.
    if maybe_phase_index.is_none() {
        return;
    }

    // Call the phase.
    let i = maybe_phase_index.unwrap();
    let called_phase_state = &mut state.phase_state[i];
    called_phase_state.is_called = true;
    called_phase_state.last_called = now;

    // If called phase is current phase, set a new timer for passage time "gap out", if needed.
    if state.current_phase == i {
        let new_green_expiration = now + signal.phases[i].passage_time;

        if new_green_expiration < state.green_must_end_at {
            scheduler.push(new_green_expiration, Command::UpdateIntersection(signal.id))
        }
    }
}

fn maybe_get_protected_phase_index(
    turn: TurnID,
    state: &TrafficSignalState,
    signal: &ControlTrafficSignal,
) -> Option<usize> {
    /* First, find the soonest phase with the turn (which might be the current phase).
    Second, if the following phase also includes the turn, advance until finding the
    last phase with the turn.
    (In edge case where every phase has the turn, just call the current phase).
    This heuristic avoids a straight-bound car calling a phase with a leading left turn
    when the following phase also has a protected straight. */

    // TODO: Build a mapping from turn group to phase in advance instead
    // of computing for every call?

    // See if there is at least one protected turn group with this turn.
    let turn_group_id = signal.turn_to_group(turn);

    let num_phases = signal.phases.len();

    let maybe_soonest_phase = signal
        .phases
        .iter()
        .cycle()
        .skip(state.current_phase)
        .take(num_phases)
        .enumerate()
        .find(|enumerated_phase| {
            TurnPriority::Protected == enumerated_phase.1.get_priority_of_group(turn_group_id)
        });

    if maybe_soonest_phase == None {
        return None;
    }

    let (num_after_current, _) = maybe_soonest_phase.unwrap();

    let soonest_phase_index = (state.current_phase + num_after_current) % num_phases;

    // Advance to the next phase without the turn group protected.
    let maybe_next_nonpriority_phase = signal
        .phases
        .iter()
        .cycle()
        .skip(soonest_phase_index)
        .take(num_phases)
        .enumerate()
        .find(|enumerated_phase| {
            TurnPriority::Protected != enumerated_phase.1.get_priority_of_group(turn_group_id)
        });

    if maybe_next_nonpriority_phase == None {
        return Some(soonest_phase_index);
    }

    let (num_after_soonest, _) = maybe_next_nonpriority_phase.unwrap();

    return Some((soonest_phase_index + num_after_soonest - 1) % num_phases);
}

fn maybe_get_yield_phase_index(
    turn: TurnID,
    state: &TrafficSignalState,
    signal: &ControlTrafficSignal,
) -> Option<usize> {
    // See if there is at least one protected turn group with this turn.
    let turn_group_id = signal.turn_to_group(turn);

    let num_phases = signal.phases.len();

    let maybe_soonest_phase = signal
        .phases
        .iter()
        .cycle()
        .skip(state.current_phase)
        .take(num_phases)
        .enumerate()
        .find(|enumerated_phase| {
            TurnPriority::Yield == enumerated_phase.1.get_priority_of_group(turn_group_id)
        });

    if maybe_soonest_phase == None {
        return None;
    }

    let (num_after_current, _) = maybe_soonest_phase.unwrap();

    let soonest_phase_index = (state.current_phase + num_after_current) % num_phases;

    // Advance to the next phase without the turn group protected.
    let maybe_next_nonyield_phase = signal
        .phases
        .iter()
        .cycle()
        .skip(soonest_phase_index)
        .take(num_phases)
        .enumerate()
        .find(|enumerated_phase| {
            TurnPriority::Banned == enumerated_phase.1.get_priority_of_group(turn_group_id)
        });

    if maybe_next_nonyield_phase == None {
        return Some(soonest_phase_index);
    }

    let (num_after_soonest, _) = maybe_next_nonyield_phase.unwrap();

    return Some((soonest_phase_index + num_after_soonest - 1) % num_phases);
}

impl TrafficSignalState {
    pub fn new(signal: &ControlTrafficSignal) -> TrafficSignalState {
        let mut state = TrafficSignalState {
            id: IntersectionID(0),
            current_phase: 0,
            phase_ends_at: Time::START_OF_DAY,
            green_must_end_at: Time::START_OF_DAY + Duration::hours(1),
            turn_group_state: BTreeMap::<TurnGroupID, TurnGroupState>::new(),
            phase_state: Vec::<PhaseState>::new(),
        };

        state.initialize(signal);

        return state;
    }

    fn initialize(&mut self, signal: &ControlTrafficSignal) {
        self.id = signal.id;

        // What phase are we starting with?
        let mut offset = signal.offset;
        loop {
            let dt = signal.phases[self.current_phase]
                .phase_type
                .simple_duration();
            if offset >= dt {
                offset -= dt;
                self.current_phase += 1;
                if self.current_phase == signal.phases.len() {
                    self.current_phase = 0;
                }
            } else {
                self.phase_ends_at = Time::START_OF_DAY + dt - offset;
                break;
            }
        }

        // Initialize turn group state
        for phase in signal.phases.iter() {
            for turn_group_id in phase.protected_groups.iter() {
                self.turn_group_state
                    .insert(*turn_group_id, TurnGroupState { is_yellow: false });
            }
            for turn_group_id in phase.yield_groups.iter() {
                self.turn_group_state
                    .insert(*turn_group_id, TurnGroupState { is_yellow: false });
            }
            self.phase_state.push(PhaseState::new());
        }
    }

    fn set_green_all_current_phase(&mut self, signal: &ControlTrafficSignal) {
        let current_phase = &signal.phases[self.current_phase];

        for turn_group in &current_phase.protected_groups {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = false;
        }

        for turn_group in &current_phase.yield_groups {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = false;
        }
    }

    fn set_yellow_all_current_phase(&mut self, signal: &ControlTrafficSignal) {
        let current_phase = &signal.phases[self.current_phase];

        for turn_group in &current_phase.protected_groups {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = true;
        }

        for turn_group in &current_phase.yield_groups {
            self.turn_group_state
                .get_mut(&turn_group)
                .unwrap()
                .is_yellow = true;
        }
    }
}
