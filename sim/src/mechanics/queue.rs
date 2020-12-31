use std::collections::{BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use abstutil::FixedMap;
use geom::{Distance, Time};
use map_model::{Map, Traversable};

use crate::mechanics::car::{Car, CarState};
use crate::{CarID, VehicleType, FOLLOWING_DISTANCE};

/// A Queue of vehicles on a single lane or turn. No over-taking or lane-changing. This is where
/// https://dabreegster.github.io/abstreet/trafficsim/discrete_event.html#exact-positions is
/// implemented.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Queue {
    pub id: Traversable,
    pub cars: VecDeque<CarID>,
    /// This car's back is still partly in this queue.
    pub laggy_head: Option<CarID>,

    pub geom_len: Distance,
    /// When a car's turn is accepted, reserve the vehicle length + FOLLOWING_DISTANCE for the
    /// target lane. When the car completely leaves (stops being the laggy_head), free up that
    /// space. To prevent blocking the box for possibly scary amounts of time, allocate some of
    /// this length first. This is unused for turns themselves. This value can exceed geom_len
    /// (for the edge case of ONE long car on a short queue).
    pub reserved_length: Distance,
}

impl Queue {
    pub fn new(id: Traversable, map: &Map) -> Queue {
        Queue {
            id,
            cars: VecDeque::new(),
            laggy_head: None,
            geom_len: id.length(map),
            reserved_length: Distance::ZERO,
        }
    }

    pub fn get_last_car_position(
        &self,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Option<(CarID, Distance)> {
        self.inner_get_last_car_position(now, cars, queues, &mut BTreeSet::new(), None)
    }

    /// Farthest along (greatest distance) is first.
    pub fn get_car_positions(
        &self,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Vec<(CarID, Distance)> {
        let mut all_cars = vec![];
        self.inner_get_last_car_position(
            now,
            cars,
            queues,
            &mut BTreeSet::new(),
            Some(&mut all_cars),
        );
        all_cars
    }

    fn inner_get_last_car_position(
        &self,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
        recursed_queues: &mut BTreeSet<Traversable>,
        mut intermediate_results: Option<&mut Vec<(CarID, Distance)>>,
    ) -> Option<(CarID, Distance)> {
        if self.cars.is_empty() {
            return None;
        }

        let mut previous: Option<(CarID, Distance)> = None;
        for id in &self.cars {
            let bound = match previous {
                Some((leader, last_dist)) => {
                    last_dist - cars[&leader].vehicle.length - FOLLOWING_DISTANCE
                }
                None => match self.laggy_head {
                    Some(id) => {
                        // The simple but broken version:
                        //self.geom_len - cars[&id].vehicle.length - FOLLOWING_DISTANCE

                        // The expensive case. We need to figure out exactly where the laggy head
                        // is on their queue.
                        let leader = &cars[&id];

                        // But don't create a cycle!
                        let recurse_to = leader.router.head();
                        if recursed_queues.contains(&recurse_to) {
                            // See the picture in
                            // https://github.com/dabreegster/abstreet/issues/30. We have two
                            // extremes to break the cycle.
                            //
                            // 1) Hope that the last person in this queue isn't bounded by the
                            //    agent in front of them yet. geom_len
                            // 2) Assume the leader has advanced minimally into the next lane.
                            //    geom_len - laggy head's length - FOLLOWING_DISTANCE.
                            //
                            // For now, optimistically assume 1. If we're wrong, consequences could
                            // be queue spillover (we're too optimistic about the number of
                            // vehicles that can fit on a lane) or cars jumping positions slightly
                            // while the cycle occurs.
                            self.geom_len
                        } else {
                            recursed_queues.insert(recurse_to);

                            let (head, head_dist) = queues[&leader.router.head()]
                                .inner_get_last_car_position(
                                    now,
                                    cars,
                                    queues,
                                    recursed_queues,
                                    None,
                                )
                                .unwrap();
                            assert_eq!(head, id);

                            let mut dist_away_from_this_queue = head_dist;
                            for on in &leader.last_steps {
                                if *on == self.id {
                                    break;
                                }
                                dist_away_from_this_queue += queues[on].geom_len;
                            }
                            // They might actually be out of the way, but laggy_head hasn't been
                            // updated yet.
                            if dist_away_from_this_queue
                                < leader.vehicle.length + FOLLOWING_DISTANCE
                            {
                                self.geom_len
                                    - (cars[&id].vehicle.length - dist_away_from_this_queue)
                                    - FOLLOWING_DISTANCE
                            } else {
                                self.geom_len
                            }
                        }
                    }
                    None => self.geom_len,
                },
            };

            // There's spillover and a car shouldn't have been able to enter yet.
            if bound < Distance::ZERO {
                if let Some(intermediate_results) = intermediate_results {
                    dump_cars(intermediate_results, cars, self.id, now);
                }
                panic!(
                    "Queue has spillover on {} at {} -- can't draw {}, bound is {}. Laggy head is \
                     {:?}. This is usually a geometry bug; check for duplicate roads going \
                     between the same intersections.",
                    self.id, now, id, bound, self.laggy_head
                );
            }

            let car = &cars[id];
            let front = match car.state {
                CarState::Queued { .. } => {
                    if car.router.last_step() {
                        car.router.get_end_dist().min(bound)
                    } else {
                        bound
                    }
                }
                CarState::WaitingToAdvance { .. } => {
                    assert_eq!(bound, self.geom_len);
                    self.geom_len
                }
                CarState::Crossing(ref time_int, ref dist_int) => {
                    // TODO Why percent_clamp_end? We process car updates in any order, so we might
                    // calculate this before moving this car from Crossing to another state.
                    dist_int.lerp(time_int.percent_clamp_end(now)).min(bound)
                }
                CarState::Unparking(front, _, _) => front,
                CarState::Parking(front, _, _) => front,
                CarState::IdlingAtStop(front, _) => front,
            };

            if let Some(ref mut intermediate_results) = intermediate_results {
                intermediate_results.push((*id, front));
            }
            previous = Some((*id, front));
        }
        // Enable to detect possible bugs, but save time otherwise
        if false {
            if let Some(intermediate_results) = intermediate_results {
                validate_positions(intermediate_results, cars, now, self.id)
            }
        }
        previous
    }

    pub fn get_idx_to_insert_car(
        &self,
        start_dist: Distance,
        vehicle_len: Distance,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Option<usize> {
        if self.laggy_head.is_none() && self.cars.is_empty() {
            return Some(0);
        }

        let dists = self.get_car_positions(now, cars, queues);
        // TODO Binary search
        let idx = match dists.iter().position(|(_, dist)| start_dist >= *dist) {
            Some(i) => i,
            None => dists.len(),
        };

        // Nope, there's not actually room at the front right now.
        if self.laggy_head.is_some() && idx == 0 {
            return None;
        }

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

    /// If true, there's room and the car must actually start the turn (because the space is
    /// reserved).
    pub fn try_to_reserve_entry(&mut self, car: &Car, force_entry: bool) -> bool {
        // If lane is already filled, then always return false, even if forced.
        if self.reserved_length >= self.geom_len {
            return false;
        }

        // Sometimes a car + FOLLOWING_DISTANCE might be longer than the geom_len entirely. In that
        // case, it just means the car won't totally fit on the queue at once, which is fine.
        // Reserve the normal amount of space; the next car trying to enter will get rejected.
        // Also allow this don't-block-the-box prevention to be disabled.
        if self.room_for_car(car) || force_entry {
            self.reserved_length += car.vehicle.length + FOLLOWING_DISTANCE;
            return true;
        }
        false
    }

    pub fn room_for_car(&self, car: &Car) -> bool {
        self.reserved_length == Distance::ZERO
            || self.reserved_length + car.vehicle.length + FOLLOWING_DISTANCE < self.geom_len
    }

    pub fn free_reserved_space(&mut self, car: &Car) {
        self.reserved_length -= car.vehicle.length + FOLLOWING_DISTANCE;
        assert!(
            self.reserved_length >= Distance::ZERO,
            "invalid reserved length: {:?}, car: {:?}",
            self.reserved_length,
            car
        );
    }

    pub fn target_lane_penalty(&self) -> (usize, usize) {
        let mut num_vehicles = self.cars.len();
        if self.laggy_head.is_some() {
            num_vehicles += 1;
        }

        let bike_cost = if self.cars.iter().any(|c| c.1 == VehicleType::Bike)
            || self
                .laggy_head
                .map(|c| c.1 == VehicleType::Bike)
                .unwrap_or(false)
        {
            1
        } else {
            0
        };

        (num_vehicles, bike_cost)
    }
}

fn validate_positions(
    dists: &Vec<(CarID, Distance)>,
    cars: &FixedMap<CarID, Car>,
    now: Time,
    id: Traversable,
) {
    for pair in dists.windows(2) {
        if pair[0].1 - cars[&pair[0].0].vehicle.length - FOLLOWING_DISTANCE < pair[1].1 {
            dump_cars(&dists, cars, id, now);
            panic!(
                "get_car_positions wound up with bad positioning: {} then {}\n{:?}",
                pair[0].1, pair[1].1, dists
            );
        }
    }
}

fn dump_cars(
    dists: &Vec<(CarID, Distance)>,
    cars: &FixedMap<CarID, Car>,
    id: Traversable,
    now: Time,
) {
    println!("\nOn {} at {}...", id, now);
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
            CarState::Queued { .. } => {
                println!("  Queued currently");
            }
            CarState::WaitingToAdvance { .. } => {
                println!("  WaitingToAdvance currently");
            }
            CarState::Unparking(_, _, ref time_int) => {
                println!("  Unparking during {} .. {}", time_int.start, time_int.end);
            }
            CarState::Parking(_, _, ref time_int) => {
                println!("  Parking during {} .. {}", time_int.start, time_int.end);
            }
            CarState::IdlingAtStop(_, ref time_int) => {
                println!("  Idling during {} .. {}", time_int.start, time_int.end);
            }
        }
    }
    println!();
}
