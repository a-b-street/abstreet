use crate::plugins::sim::new_des_model::{
    Car, CarState, FOLLOWING_DISTANCE, FREEFLOW, VEHICLE_LENGTH, WAITING,
};
use geom::{Distance, Duration};
use map_model::{LaneID, Map};
use sim::DrawCarInput;
use std::collections::VecDeque;

pub struct Queue {
    pub id: LaneID,
    pub cars: VecDeque<Car>,
    pub max_capacity: usize,
}

impl Queue {
    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.cars.len() == self.max_capacity
    }

    pub fn get_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        if self.cars.is_empty() {
            return Vec::new();
        }
        let l = map.get_l(self.id);

        let mut result: Vec<DrawCarInput> = Vec::new();
        let mut last_car_back: Option<Distance> = None;

        for car in &self.cars {
            let (front, color) = match car.state {
                CarState::Queued => {
                    if last_car_back.is_none() {
                        (l.length(), WAITING)
                    } else {
                        // TODO If the last car is still CrossingLane, then kinda weird to draw
                        // us as queued
                        (last_car_back.unwrap() - FOLLOWING_DISTANCE, WAITING)
                    }
                }
                CarState::CrossingLane(ref i) => {
                    let bound = last_car_back
                        .map(|b| b - FOLLOWING_DISTANCE)
                        .unwrap_or_else(|| l.length());
                    (i.percent(time) * bound, FREEFLOW)
                }
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

            if let Some(d) = car.get_draw_car(front, color, map) {
                result.push(d);
            }
            last_car_back = Some(front - VEHICLE_LENGTH);
        }
        result
    }
}
