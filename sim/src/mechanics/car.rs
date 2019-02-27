use crate::{CarStatus, DrawCarInput};
use crate::{DistanceInterval, ParkingSpot, Router, TimeInterval, Vehicle};
use geom::{Distance, Duration, PolyLine};
use map_model::{Map, Traversable, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Car {
    pub vehicle: Vehicle,
    pub state: CarState,
    pub router: Router,

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
        let on = self.router.head();
        let dist_int = DistanceInterval::new_driving(
            start_dist,
            if self.router.last_step() {
                self.router.get_end_dist()
            } else {
                on.length(map)
            },
        );
        let mut speed = on.speed_limit(map);
        if let Some(s) = self.vehicle.max_speed {
            speed = speed.min(s);
        }
        let dt = (dist_int.end - dist_int.start) / speed;
        CarState::Crossing(TimeInterval::new(start_time, start_time + dt), dist_int)
    }

    pub fn trim_last_steps(&mut self, map: &Map) {
        let mut keep = VecDeque::new();
        let mut len = Distance::ZERO;
        for on in self.last_steps.drain(..) {
            len += on.length(map);
            keep.push_back(on);
            if len >= self.vehicle.length {
                break;
            }
        }
        self.last_steps = keep;
    }

    pub fn get_draw_car(&self, front: Distance, time: Duration, map: &Map) -> DrawCarInput {
        assert!(front >= Distance::ZERO);
        let raw_body = if front >= self.vehicle.length {
            self.router
                .head()
                .slice(front - self.vehicle.length, front, map)
                .unwrap()
                .0
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
                    panic!("{} spawned too close to short stuff", self.vehicle.id);
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
            waiting_for_turn: None,
            stopping_trace: None,
            status: match self.state {
                // TODO Cars can be Queued behind a slow Crossing. Looks kind of weird.
                CarState::Queued => CarStatus::Stuck,
                CarState::Crossing(_, _) => CarStatus::Moving,
                // Eh they're technically moving, but this is a bit easier to spot
                CarState::Unparking(_, _) => CarStatus::Parked,
                CarState::Parking(_, _, _) => CarStatus::Parked,
            },
            vehicle_type: self.vehicle.vehicle_type,
            on: self.router.head(),
            body,
        }
    }

    pub fn is_queued(&self) -> bool {
        match self.state {
            CarState::Queued => true,
            _ => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum CarState {
    // TODO These two should perhaps be collapsed to (TimeInterval, DistanceInterval, Traversable).
    Crossing(TimeInterval, DistanceInterval),
    Queued,
    // Where's the front of the car while this is happening?
    Unparking(Distance, TimeInterval),
    Parking(Distance, ParkingSpot, TimeInterval),
}
