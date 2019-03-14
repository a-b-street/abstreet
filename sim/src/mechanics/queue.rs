use crate::mechanics::car::{Car, CarState};
use crate::{CarID, FOLLOWING_DISTANCE};
use geom::{Distance, Duration};
use map_model::{Map, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Queue {
    pub id: Traversable,
    pub cars: VecDeque<CarID>,

    pub geom_len: Distance,
}

impl Queue {
    pub fn new(id: Traversable, map: &Map) -> Queue {
        let len = id.length(map);
        Queue {
            id,
            cars: VecDeque::new(),
            geom_len: len,
        }
    }

    // Farthest along (greatest distance) is first.
    pub fn get_car_positions(
        &self,
        time: Duration,
        cars: &BTreeMap<CarID, Car>,
    ) -> Vec<(CarID, Distance)> {
        if self.cars.is_empty() {
            return Vec::new();
        }

        let mut result: Vec<(CarID, Distance)> = Vec::new();

        for id in &self.cars {
            let bound = match result.last() {
                Some((leader, last_dist)) => {
                    *last_dist - cars[leader].vehicle.length - FOLLOWING_DISTANCE
                }
                None => self.geom_len,
            };

            // There's spillover and a car shouldn't have been able to enter yet.
            if bound < Distance::ZERO {
                dump_cars(&result, cars, self.id, time);
                panic!(
                    "Queue has spillover on {:?} at {} -- can't draw {}, bound is {}",
                    self.id, time, id, bound
                );
            }

            let car = &cars[id];
            let front = match car.state {
                CarState::Queued => {
                    if car.router.last_step() {
                        car.router.get_end_dist().min(bound)
                    } else {
                        bound
                    }
                }
                CarState::WaitingToAdvance => {
                    assert_eq!(bound, self.geom_len);
                    self.geom_len
                }
                CarState::Crossing(ref time_int, ref dist_int) => {
                    // TODO Why percent_clamp_end? We process car updates in any order, so we might
                    // calculate this before moving this car from Crossing to another state.
                    dist_int.lerp(time_int.percent_clamp_end(time)).min(bound)
                }
                CarState::Unparking(front, _) => front,
                CarState::Parking(front, _, _) => front,
                CarState::Idling(front, _) => front,
            };

            result.push((*id, front));
        }
        validate_positions(result, cars, time, self.id)
    }

    pub fn get_idx_to_insert_car(
        &self,
        start_dist: Distance,
        vehicle_len: Distance,
        time: Duration,
        cars: &BTreeMap<CarID, Car>,
    ) -> Option<usize> {
        if self.cars.is_empty() {
            return Some(0);
        }

        let dists = self.get_car_positions(time, cars);
        // TODO Binary search
        let idx = match dists.iter().position(|(_, dist)| start_dist >= *dist) {
            Some(i) => i,
            None => dists.len(),
        };

        // Are we too close to the leader?
        if idx != 0
            && dists[idx - 1].1 - cars[&dists[idx - 1].0].vehicle.length - FOLLOWING_DISTANCE
                < start_dist
        {
            return None;
        }
        // Or the follower?
        if idx != dists.len() && start_dist - vehicle_len - FOLLOWING_DISTANCE < dists[idx].1 {
            return None;
        }

        Some(idx)
    }

    pub fn room_at_end(&self, time: Duration, cars: &BTreeMap<CarID, Car>) -> bool {
        // This could also be implemented by calling get_idx_to_insert_car with start_dist =
        // self.geom_len
        match self.get_car_positions(time, cars).last() {
            Some((car, front)) => *front >= cars[car].vehicle.length + FOLLOWING_DISTANCE,
            None => true,
        }
    }
}

fn validate_positions(
    dists: Vec<(CarID, Distance)>,
    cars: &BTreeMap<CarID, Car>,
    time: Duration,
    id: Traversable,
) -> Vec<(CarID, Distance)> {
    for pair in dists.windows(2) {
        if pair[0].1 - cars[&pair[0].0].vehicle.length - FOLLOWING_DISTANCE < pair[1].1 {
            dump_cars(&dists, cars, id, time);
            panic!(
                "get_car_positions wound up with bad positioning: {} then {}\n{:?}",
                pair[0].1, pair[1].1, dists
            );
        }
    }
    dists
}

fn dump_cars(
    dists: &Vec<(CarID, Distance)>,
    cars: &BTreeMap<CarID, Car>,
    id: Traversable,
    time: Duration,
) {
    println!("\nOn {:?} at {}...", id, time);
    for (id, dist) in dists {
        let car = &cars[id];
        println!("- {} @ {} (length {})", id, dist, car.vehicle.length);
        match car.state {
            CarState::Crossing(ref time_int, ref dist_int) => {
                println!(
                    "  Going {} .. {} during {} .. {}",
                    dist_int.start, dist_int.end, time_int.start, time_int.end
                );
            }
            CarState::Queued => {
                println!("  Queued currently");
            }
            CarState::WaitingToAdvance => {
                println!("  WaitingToAdvance currently");
            }
            CarState::Unparking(_, ref time_int) => {
                println!("  Unparking during {} .. {}", time_int.start, time_int.end);
            }
            CarState::Parking(_, _, ref time_int) => {
                println!("  Parking during {} .. {}", time_int.start, time_int.end);
            }
            CarState::Idling(_, ref time_int) => {
                println!("  Idling during {} .. {}", time_int.start, time_int.end);
            }
        }
    }
    println!();
}
