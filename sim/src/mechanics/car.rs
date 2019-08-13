use crate::{
    CarStatus, DistanceInterval, DrawCarInput, ParkingSpot, Router, TimeInterval, TransitSimState,
    TripID, Vehicle, VehicleType,
};
use geom::{Distance, Duration, PolyLine};
use map_model::{Map, Traversable, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Car {
    pub vehicle: Vehicle,
    pub state: CarState,
    pub router: Router,
    pub trip: TripID,
    pub blocked_since: Option<Duration>,

    // In reverse order -- most recently left is first. The sum length of these must be >=
    // vehicle.length.
    pub last_steps: VecDeque<Traversable>,
}

impl Car {
    // Assumes the current head of the path is the thing to cross.
    pub fn crossing_state(
        &self,
        start_dist: Distance,
        start_time: Duration,
        map: &Map,
    ) -> CarState {
        let dist_int = DistanceInterval::new_driving(
            start_dist,
            if self.router.last_step() {
                self.router.get_end_dist()
            } else {
                self.router.head().length(map)
            },
        );
        self.crossing_state_with_end_dist(dist_int, start_time, map)
    }

    pub fn crossing_state_with_end_dist(
        &self,
        dist_int: DistanceInterval,
        start_time: Duration,
        map: &Map,
    ) -> CarState {
        let on = self.router.head();
        let mut speed = on.speed_limit(map);
        if let Some(s) = self.vehicle.max_speed {
            speed = speed.min(s);
        }
        let dt = (dist_int.end - dist_int.start) / speed;
        CarState::Crossing(TimeInterval::new(start_time, start_time + dt), dist_int)
    }

    pub fn get_draw_car(
        &self,
        front: Distance,
        time: Duration,
        map: &Map,
        transit: &TransitSimState,
    ) -> DrawCarInput {
        assert!(front >= Distance::ZERO);
        let raw_body = if front >= self.vehicle.length {
            self.router
                .head()
                .exact_slice(front - self.vehicle.length, front, map)
        } else {
            // TODO This is redoing some of the Path::trace work...
            let mut result = self
                .router
                .head()
                .slice(Distance::ZERO, front, map)
                .map(|(pl, _)| pl.points().clone())
                .unwrap_or_else(Vec::new);
            let mut leftover = self.vehicle.length - front;
            let mut i = 0;
            while leftover > Distance::ZERO {
                if i == self.last_steps.len() {
                    panic!(
                        "{} spawned too close to short stuff; still need to account for {}",
                        self.vehicle.id, leftover
                    );
                }
                let len = self.last_steps[i].length(map);
                let start = (len - leftover).max(Distance::ZERO);
                let piece = self.last_steps[i]
                    .slice(start, len, map)
                    .map(|(pl, _)| pl.points().clone())
                    .unwrap_or_else(Vec::new);
                result = PolyLine::append(piece, result);
                leftover -= len;
                i += 1;
            }

            PolyLine::new(result)
        };

        let body = match self.state {
            // Assume the parking lane is to the right of us!
            CarState::Unparking(_, ref time_int) => raw_body
                .shift_right(LANE_THICKNESS * (1.0 - time_int.percent(time)))
                .unwrap(),
            CarState::Parking(_, _, ref time_int) => raw_body
                .shift_right(LANE_THICKNESS * time_int.percent(time))
                .unwrap(),
            _ => raw_body,
        };

        DrawCarInput {
            id: self.vehicle.id,
            waiting_for_turn: match self.state {
                // TODO Maybe also when Crossing?
                CarState::WaitingToAdvance | CarState::Queued => match self.router.maybe_next() {
                    Some(Traversable::Turn(t)) => Some(t),
                    _ => None,
                },
                _ => None,
            },
            status: match self.state {
                // TODO Cars can be Queued behind a slow Crossing. Looks kind of weird.
                CarState::Queued => CarStatus::Stuck,
                CarState::WaitingToAdvance => CarStatus::Stuck,
                CarState::Crossing(_, _) => CarStatus::Moving,
                // Eh they're technically moving, but this is a bit easier to spot
                CarState::Unparking(_, _) => CarStatus::Parked,
                CarState::Parking(_, _, _) => CarStatus::Parked,
                // Changing color for idling buses is helpful
                CarState::Idling(_, _) => CarStatus::Parked,
            },
            on: self.router.head(),
            label: if self.vehicle.vehicle_type == VehicleType::Bus {
                Some(
                    map.get_br(transit.bus_route(self.vehicle.id))
                        .name
                        .to_string(),
                )
            } else {
                None
            },
            body,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum CarState {
    Crossing(TimeInterval, DistanceInterval),
    Queued,
    WaitingToAdvance,
    // Where's the front of the car while this is happening?
    Unparking(Distance, TimeInterval),
    Parking(Distance, ParkingSpot, TimeInterval),
    Idling(Distance, TimeInterval),
}

impl CarState {
    pub fn get_end_time(&self) -> Duration {
        match self {
            CarState::Crossing(ref time_int, _) => time_int.end,
            CarState::Queued => unreachable!(),
            CarState::WaitingToAdvance => unreachable!(),
            CarState::Unparking(_, ref time_int) => time_int.end,
            CarState::Parking(_, _, ref time_int) => time_int.end,
            CarState::Idling(_, ref time_int) => time_int.end,
        }
    }
}
