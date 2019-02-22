use crate::plugins::sim::new_des_model::Vehicle;
use geom::{Distance, Duration, PolyLine};
use map_model::{Map, Traversable};
use sim::DrawCarInput;
use std::collections::VecDeque;

#[derive(Debug)]
pub struct Car {
    pub vehicle: Vehicle,
    // Front is always the current step
    pub path: VecDeque<Traversable>,
    pub end_dist: Distance,
    pub state: CarState,

    // In reverse order -- most recently left is first. The sum length of these must be >=
    // vehicle.length.
    pub last_steps: VecDeque<Traversable>,
}

impl Car {
    pub fn crossing_state(
        &self,
        start_dist: Distance,
        start_time: Duration,
        on: Traversable,
        map: &Map,
    ) -> CarState {
        let dist_int = DistanceInterval::new(
            start_dist,
            if self.path.len() == 1 {
                self.end_dist
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

    pub fn get_draw_car(&self, front: Distance, map: &Map) -> DrawCarInput {
        assert!(front >= Distance::ZERO);
        let body = if front >= self.vehicle.length {
            self.path[0]
                .slice(front - self.vehicle.length, front, map)
                .unwrap()
                .0
        } else {
            // TODO This is redoing some of the Path::trace work...
            let mut result = self.path[0]
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

        DrawCarInput {
            id: self.vehicle.id,
            waiting_for_turn: None,
            stopping_trace: None,
            state: match self.state {
                // TODO Cars can be Queued behind a slow Crossing. Looks kind of weird.
                CarState::Queued => sim::CarState::Stuck,
                CarState::Crossing(_, _) => sim::CarState::Moving,
            },
            vehicle_type: self.vehicle.vehicle_type,
            on: self.path[0],
            body,
        }
    }
}

// TODO These should perhaps be collapsed to (TimeInterval, DistanceInterval, Traversable).
#[derive(Debug)]
pub enum CarState {
    Crossing(TimeInterval, DistanceInterval),
    Queued,
}

#[derive(Debug)]
pub struct TimeInterval {
    // TODO Private fields
    pub start: Duration,
    pub end: Duration,
}

impl TimeInterval {
    fn new(start: Duration, end: Duration) -> TimeInterval {
        if end < start {
            panic!("Bad TimeInterval {} .. {}", start, end);
        }
        TimeInterval { start, end }
    }

    pub fn percent(&self, t: Duration) -> f64 {
        if self.start == self.end {
            return 1.0;
        }

        let x = (t - self.start) / (self.end - self.start);
        assert!(x >= 0.0 && x <= 1.0);
        x
    }
}

#[derive(Debug)]
pub struct DistanceInterval {
    // TODO Private fields
    pub start: Distance,
    pub end: Distance,
}

impl DistanceInterval {
    fn new(start: Distance, end: Distance) -> DistanceInterval {
        if end < start {
            panic!("Bad DistanceInterval {} .. {}", start, end);
        }
        DistanceInterval { start, end }
    }

    pub fn lerp(&self, x: f64) -> Distance {
        assert!(x >= 0.0 && x <= 1.0);
        self.start + x * (self.end - self.start)
    }
}
