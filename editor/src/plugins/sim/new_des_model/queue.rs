use crate::plugins::sim::new_des_model::{Car, CarState, FOLLOWING_DISTANCE, VEHICLE_LENGTH};
use geom::{Distance, Duration};
use map_model::LaneID;
use std::collections::VecDeque;

pub struct Queue {
    pub id: LaneID,
    pub cars: VecDeque<Car>,
    pub max_capacity: usize,

    pub lane_len: Distance,
}

impl Queue {
    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.cars.len() == self.max_capacity
    }

    // May not return all of the cars -- some might be temporarily unable to actually enter the end
    // of the road.
    pub fn get_car_positions(&self, time: Duration) -> Vec<(&Car, Distance)> {
        if self.cars.is_empty() {
            return Vec::new();
        }

        let mut result: Vec<(&Car, Distance)> = Vec::new();

        for car in &self.cars {
            let bound = match result.last() {
                Some((_, last_dist)) => *last_dist - VEHICLE_LENGTH - FOLLOWING_DISTANCE,
                None => self.lane_len,
            };

            let front = match car.state {
                CarState::Queued => bound,
                CarState::CrossingLane(ref i) => i.percent(time) * bound,
                CarState::CrossingTurn(_) => unreachable!(),
            };

            // There's backfill and a car shouldn't have been able to enter yet, but it's a
            // temporary condition -- there's enough rest capacity.
            if front < Distance::ZERO {
                println!(
                    "Queue temporarily backed up on {} -- can't draw {}",
                    self.id, car.id
                );
                return result;
            }
            result.push((car, front));
        }
        result
    }
}
