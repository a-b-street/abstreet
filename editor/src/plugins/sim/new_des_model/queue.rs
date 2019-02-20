use crate::plugins::sim::new_des_model::{Car, CarState, FOLLOWING_DISTANCE, MAX_VEHICLE_LENGTH};
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
    // May not return all of the cars -- some might be temporarily unable to actually enter the end
    // of the road.
    // Farthest along (greatest distance) is first.
    pub fn get_car_positions(&self, time: Duration) -> Vec<(&Car, Distance)> {
        if self.cars.is_empty() {
            return Vec::new();
        }

        let mut result: Vec<(&Car, Distance)> = Vec::new();

        for car in &self.cars {
            let bound = match result.last() {
                Some((leader, last_dist)) => *last_dist - leader.vehicle_len - FOLLOWING_DISTANCE,
                None => self.lane_len,
            };

            // There's spillover and a car shouldn't have been able to enter yet.
            if bound < Distance::ZERO {
                dump_cars(&result, self.id, time);
                panic!(
                    "Queue has spillover on {} at {} -- can't draw {}, bound is {}",
                    self.id, time, car.id, bound
                );
            }

            let front = match car.state {
                CarState::Queued => bound,
                CarState::CrossingLane(ref time_int, ref dist_int) => {
                    dist_int.lerp(time_int.percent(time)).min(bound)
                }
                CarState::CrossingTurn(_) => unreachable!(),
            };

            result.push((car, front));
        }
        validate_positions(result, time, self.id)
    }

    pub fn get_idx_to_insert_car(&self, start_dist: Distance, time: Duration) -> Option<usize> {
        if self.cars.len() == self.max_capacity {
            return None;
        }
        if self.cars.is_empty() {
            return Some(0);
        }

        let dists = self.get_car_positions(time);
        // TODO Binary search
        let idx = match dists.iter().position(|(_, dist)| start_dist <= *dist) {
            Some(i) => i + 1,
            None => 0,
        };

        // Are we too close to the leader?
        if idx != 0
            && dists[idx - 1].1 - dists[idx - 1].0.vehicle_len - FOLLOWING_DISTANCE < start_dist
        {
            return None;
        }
        // Or the follower?
        if idx != dists.len() && start_dist - MAX_VEHICLE_LENGTH - FOLLOWING_DISTANCE < dists[idx].1
        {
            return None;
        }

        Some(idx)
    }

    pub fn room_at_end(&self, time: Duration) -> bool {
        match self.get_car_positions(time).last() {
            Some((_, front)) => *front >= MAX_VEHICLE_LENGTH + FOLLOWING_DISTANCE,
            None => true,
        }
    }
}

fn validate_positions(
    cars: Vec<(&Car, Distance)>,
    time: Duration,
    id: LaneID,
) -> Vec<(&Car, Distance)> {
    for pair in cars.windows(2) {
        if pair[0].1 - pair[0].0.vehicle_len - FOLLOWING_DISTANCE < pair[1].1 {
            dump_cars(&cars, id, time);
            panic!(
                "get_car_positions wound up with bad positioning: {} then {}\n{:?}",
                pair[0].1, pair[1].1, cars
            );
        }
    }
    cars
}

fn dump_cars(cars: &Vec<(&Car, Distance)>, id: LaneID, time: Duration) {
    println!("\nOn {} at {}...", id, time);
    for (car, dist) in cars {
        println!("- {} @ {} (length {})", car.id, dist, car.vehicle_len);
        match car.state {
            CarState::CrossingLane(ref time_int, ref dist_int) => {
                println!(
                    "  Going {} .. {} during {} .. {}",
                    dist_int.start, dist_int.end, time_int.start, time_int.end
                );
            }
            CarState::Queued => {
                println!("  Queued currently");
            }
            CarState::CrossingTurn(_) => unreachable!(),
        }
    }
    println!();
}
