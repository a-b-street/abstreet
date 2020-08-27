use crate::{Command, Scheduler};
use geom::{Time, Duration};
use map_model::{ControlTrafficSignal, IntersectionID, TrafficControlType, TurnGroupID};
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
    turn_group_state: BTreeMap<TurnGroupID, TurnGroupState>,
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

impl TrafficSignalState {
    pub fn new(signal: &ControlTrafficSignal) -> TrafficSignalState {
        let mut state = TrafficSignalState {
            id: IntersectionID(0),
            current_phase: 0,
            phase_ends_at: Time::START_OF_DAY,
            turn_group_state: BTreeMap::<TurnGroupID, TurnGroupState>::new(),
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
    }

    fn set_green_all_current_phase(&mut self, signal: &ControlTrafficSignal) {
        let current_phase = &signal.phases[self.current_phase];

        for turn_group in &current_phase.protected_groups {
            self.turn_group_state.get_mut(&turn_group).unwrap().is_yellow = false;
        }

        for turn_group in &current_phase.yield_groups {
            self.turn_group_state.get_mut(&turn_group).unwrap().is_yellow = false;
        }
    }

    
    fn set_yellow_all_current_phase(&mut self, signal: &ControlTrafficSignal) {
        let current_phase = &signal.phases[self.current_phase];

        for turn_group in &current_phase.protected_groups {
            self.turn_group_state.get_mut(&turn_group).unwrap().is_yellow = true;
        }

        for turn_group in &current_phase.yield_groups {
            self.turn_group_state.get_mut(&turn_group).unwrap().is_yellow = true;
        }
    }
}