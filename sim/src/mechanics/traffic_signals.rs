use crate::{Command, Scheduler};
use geom::Time;
use map_model::{ControlTrafficSignal, IntersectionID, TrafficControlType, TurnGroupID};
use serde::{Deserialize, Serialize};

type Call = (TurnGroupID, Time);

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct TrafficSignalState {
    pub current_phase: usize,
    pub phase_ends_at: Time,
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
        TrafficControlType::Original => unreachable!(),
    }
}

fn update_pretimed(
    now: Time,
    intersection_id: IntersectionID,
    state: &mut TrafficSignalState,
    signal: &ControlTrafficSignal,
    scheduler: &mut Scheduler,
) {
    if now == state.phase_ends_at {
        state.current_phase = (state.current_phase + 1) % signal.phases.len();

        state.phase_ends_at = now
            + signal.phases[state.current_phase]
                .phase_type
                .simple_duration();
        scheduler.push(
            state.phase_ends_at,
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
}
