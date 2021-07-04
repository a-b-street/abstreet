use std::collections::{BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use abstutil::FixedMap;
use geom::{Distance, Time};
use map_model::{Map, Position, Traversable};

use crate::mechanics::car::{Car, CarState};
use crate::{CarID, VehicleType, FOLLOWING_DISTANCE};

/// A Queue of vehicles on a single lane or turn. This is where
/// https://a-b-street.github.io/docs/tech/trafficsim/discrete_event.html#exact-positions is
/// implemented.
///
/// Some helpful pieces of terminology:
///
/// - a "laggy head" is a vehicle whose front is now past the end of this queue, but whose back may
///   still be partially in the queue. The position of the first car in the queue is still bounded
///   by the laggy head's back.
/// - a "static blockage" is due to a vehicle exiting a driveway and immediately cutting across a
///   few lanes. The "static" part means it occupies a fixed interval of distance in the queue. When
///   the vehicle is finished exiting the driveway, this blockage is removed.
/// - a "dynamic blockage" is due to a vehicle changing lanes in the middle of the queue. The exact
///   position of the blockage in this queue is unknown (it depends on the target queue). The
///   blockage just occupies the length of the vehicle and keeps following whatever's in front of
///   it.
/// - "active cars" are the main members of the queue -- everything except for laggy heads and
///   blockages.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Queue {
    pub id: Traversable,
    members: VecDeque<Queued>,
    /// This car's back is still partly in this queue.
    pub laggy_head: Option<CarID>,

    /// How long the lane or turn physically is.
    pub geom_len: Distance,
    /// When a car's turn is accepted, reserve the vehicle length + FOLLOWING_DISTANCE for the
    /// target lane. When the car completely leaves (stops being the laggy_head), free up that
    /// space. To prevent blocking the box for possibly scary amounts of time, allocate some of
    /// this length first. This is unused for turns themselves. This value can exceed geom_len
    /// (for the edge case of ONE long car on a short queue).
    pub reserved_length: Distance,
}

/// A member of a `Queue`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Queued {
    /// A regular vehicle trying to move forwards
    Vehicle(CarID),
    /// Something occupying a fixed interval of distance on the queue
    StaticBlockage {
        /// This vehicle is exiting a driveway and cutting across a few lanes
        cause: CarID,
        front: Distance,
        back: Distance,
    },
    /// This follows whatever's in front of it
    DynamicBlockage {
        /// This vehicle is in the middle of changing lanes
        cause: CarID,
        vehicle_len: Distance,
    },
}

/// The exact position of something in a `Queue` at some time
#[derive(Clone, Debug)]
pub struct QueueEntry {
    pub member: Queued,
    pub front: Distance,
    /// Not incuding FOLLOWING_DISTANCE
    pub back: Distance,
}

impl Queue {
    pub fn new(id: Traversable, map: &Map) -> Queue {
        Queue {
            id,
            members: VecDeque::new(),
            laggy_head: None,
            geom_len: id.get_polyline(map).length(),
            reserved_length: Distance::ZERO,
        }
    }

    /// Get the front of the last car in the queue.
    pub fn get_last_car_position(
        &self,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Option<(CarID, Distance)> {
        self.inner_get_last_car_position(now, cars, queues, &mut BTreeSet::new(), None)
    }

    /// Return the exact position of each member of the queue. The farthest along (greatest distance) is first.
    pub fn get_car_positions(
        &self,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Vec<QueueEntry> {
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

    /// Returns the front of the last car in the queue, only if the last member is an active car.
    fn inner_get_last_car_position(
        &self,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
        recursed_queues: &mut BTreeSet<Traversable>,
        mut intermediate_results: Option<&mut Vec<QueueEntry>>,
    ) -> Option<(CarID, Distance)> {
        if self.members.is_empty() {
            return None;
        }

        // TODO Consider simplifying this loop's structure. Calculate the bound here before
        // starting the loop, handling the laggy head case.
        let mut previous: Option<QueueEntry> = None;
        for queued in self.members.iter().cloned() {
            let bound = match previous {
                Some(entry) => entry.back - FOLLOWING_DISTANCE,
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
                            // https://github.com/a-b-street/abstreet/issues/30. We have two
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
                    "Queue has spillover on {} at {} -- can't draw {:?}, bound is {}. Laggy head is \
                     {:?}. This is usually a geometry bug; check for duplicate roads going \
                     between the same intersections.",
                    self.id, now, queued, bound, self.laggy_head
                );
            }

            let entry = match queued {
                Queued::Vehicle(id) => {
                    let car = &cars[&id];
                    let front = match car.state {
                        CarState::Queued { .. } => {
                            if car.router.last_step() {
                                car.router.get_end_dist().min(bound)
                            } else {
                                bound
                            }
                        }
                        CarState::WaitingToAdvance { .. } => {
                            if bound != self.geom_len {
                                if let Some(intermediate_results) = intermediate_results {
                                    dump_cars(intermediate_results, cars, self.id, now);
                                }
                                panic!("{} is waiting to advance on {}, but the current bound is {}, not geom_len {}. How can anything be in front of them?", id, self.id, bound, self.geom_len);
                            }
                            self.geom_len
                        }
                        CarState::Crossing(ref time_int, ref dist_int) => {
                            // TODO Why percent_clamp_end? We process car updates in any order, so we might
                            // calculate this before moving this car from Crossing to another state.
                            dist_int.lerp(time_int.percent_clamp_end(now)).min(bound)
                        }
                        CarState::ChangingLanes {
                            ref new_time,
                            ref new_dist,
                            ..
                        } => {
                            // Same as the Crossing logic
                            new_dist.lerp(new_time.percent_clamp_end(now)).min(bound)
                        }
                        CarState::Unparking { front, .. } => front,
                        CarState::Parking(front, _, _) => front,
                        CarState::IdlingAtStop(front, _) => front,
                    };
                    QueueEntry {
                        member: queued,
                        front,
                        back: front - car.vehicle.length,
                    }
                }
                Queued::StaticBlockage { front, back, .. } => QueueEntry {
                    member: queued,
                    front,
                    back,
                },
                Queued::DynamicBlockage { vehicle_len, .. } => QueueEntry {
                    member: queued,
                    // This is a reasonable guess, because a vehicle only starts changing lanes if
                    // there's something slower in front of it. So we assume that slow vehicle
                    // continues to exist for the 1 second that lane-changing takes. If for some
                    // reason that slower leader vanishes, this bound could jump up, which just
                    // causes anything following the lane-changing vehicle to be able to go a
                    // little faster.
                    front: bound,
                    back: bound - vehicle_len,
                },
            };

            if let Some(ref mut intermediate_results) = intermediate_results {
                intermediate_results.push(entry.clone());
            }
            previous = Some(entry);
        }
        // Enable to detect possible bugs, but save time otherwise
        if false {
            if let Some(intermediate_results) = intermediate_results {
                validate_positions(intermediate_results, cars, now, self.id)
            }
        }

        let previous = previous?;
        match previous.member {
            Queued::Vehicle(car) => Some((car, previous.front)),
            Queued::StaticBlockage { .. } => None,
            Queued::DynamicBlockage { .. } => None,
        }
    }

    /// If the specified car can appear in the queue, return the position in the queue to do so.
    pub fn get_idx_to_insert_car(
        &self,
        start_dist: Distance,
        vehicle_len: Distance,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Option<usize> {
        if self.laggy_head.is_none() && self.members.is_empty() {
            return Some(0);
        }

        let dists = self.get_car_positions(now, cars, queues);
        // TODO Binary search
        let idx = match dists.iter().position(|entry| start_dist >= entry.front) {
            Some(i) => i,
            None => dists.len(),
        };

        // Nope, there's not actually room at the front right now.
        // (This is overly conservative; we could figure out exactly where the laggy head is and
        // maybe allow it.)
        if idx == 0 {
            if let Some(c) = self.laggy_head {
                // We don't know exactly where the laggy head is. So assume the worst case; that
                // they've just barely started the turn, and we have to use the same
                // too-close-to-leader math.
                //
                // TODO We can be more precise! We already call get_car_positions, and that
                // calculates exactly where the laggy head is. We just need to plumb that bound
                // back here.
                if self.geom_len - cars[&c].vehicle.length - FOLLOWING_DISTANCE < start_dist {
                    return None;
                }
            }
        }

        // Are we too close to the leader?
        if idx != 0 && dists[idx - 1].back - FOLLOWING_DISTANCE < start_dist {
            return None;
        }
        // Or the follower?
        if idx != dists.len() && start_dist - vehicle_len - FOLLOWING_DISTANCE < dists[idx].front {
            return None;
        }

        Some(idx)
    }

    /// Record that a car has entered a queue at a position. This must match get_idx_to_insert_car
    /// -- the same index and immediately after passing that query.
    pub fn insert_car_at_idx(&mut self, idx: usize, car: &Car) {
        self.members.insert(idx, Queued::Vehicle(car.vehicle.id));
        self.reserved_length += car.vehicle.length + FOLLOWING_DISTANCE;
    }

    /// Record that a car has entered a queue at the end. It's assumed that try_to_reserve_entry
    /// has already happened.
    pub fn push_car_onto_end(&mut self, car: CarID) {
        self.members.push_back(Queued::Vehicle(car));
    }

    /// Change the first car in the queue to the laggy head, indicating that it's front has left
    /// the queue, but its back is still there. Return that car.
    pub fn move_first_car_to_laggy_head(&mut self) -> CarID {
        assert!(self.laggy_head.is_none());
        let car = match self.members.pop_front() {
            Some(Queued::Vehicle(c)) => c,
            x => {
                panic!(
                    "First member of {} is {:?}, not an active vehicle",
                    self.id, x
                );
            }
        };
        self.laggy_head = Some(car);
        car
    }

    /// If true, there's room and the car must actually start the turn (because the space is
    /// reserved).
    pub fn try_to_reserve_entry(&mut self, car: &Car, force_entry: bool) -> bool {
        // If self.reserved_length >= self.geom_len, then the lane is already full. Normally we
        // won't allow more cars to start a turn towards it, but if force_entry is true, then we'll
        // allow it.

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

    /// True if the reserved length exceeds the physical length. This means a vehicle is headed
    /// towards the queue already and is expected to not fit entirely inside.
    pub fn is_overflowing(&self) -> bool {
        self.reserved_length >= self.geom_len
    }

    /// Can a car start a turn for this queue?
    pub fn room_for_car(&self, car: &Car) -> bool {
        self.reserved_length == Distance::ZERO
            || self.reserved_length + car.vehicle.length + FOLLOWING_DISTANCE < self.geom_len
    }

    /// Once a car has fully exited a queue, free up the space it was reserving.
    pub fn free_reserved_space(&mut self, car: &Car) {
        self.reserved_length -= car.vehicle.length + FOLLOWING_DISTANCE;
        assert!(
            self.reserved_length >= Distance::ZERO,
            "invalid reserved length: {:?}, car: {:?}",
            self.reserved_length,
            car
        );
    }

    /// Return a penalty for entering this queue, as opposed to some adjacent ones. Used for
    /// lane-changing. (number of vehicles, is there a bike here)
    pub fn target_lane_penalty(&self) -> (usize, usize) {
        let mut num_vehicles = self.members.len();
        if self.laggy_head.is_some() {
            num_vehicles += 1;
        }

        let bike_cost = if self
            .members
            .iter()
            .any(|x| matches!(x, Queued::Vehicle(c) if c.vehicle_type == VehicleType::Bike))
            || self
                .laggy_head
                .map(|c| c.vehicle_type == VehicleType::Bike)
                .unwrap_or(false)
        {
            1
        } else {
            0
        };

        (num_vehicles, bike_cost)
    }

    /// Find the vehicle in front of the specified input. None if the specified vehicle isn't
    /// ACTIVE (not a blockage) in the queue at all, or they're the front (with or without a laggy
    /// head).
    pub fn get_leader(&self, id: CarID) -> Option<CarID> {
        let mut leader = None;
        for queued in &self.members {
            match queued {
                Queued::Vehicle(car) => {
                    if *car == id {
                        return leader;
                    }
                    leader = Some(*car);
                }
                Queued::StaticBlockage { .. } | Queued::DynamicBlockage { .. } => {
                    leader = None;
                }
            }
        }
        None
    }

    /// Record that a car is blocking a static portion of the queue (from front to back). Must use
    /// the index from can_block_from_driveway.
    pub fn add_static_blockage(
        &mut self,
        cause: CarID,
        front: Distance,
        back: Distance,
        idx: usize,
    ) {
        assert!(front > back);
        assert!(back >= FOLLOWING_DISTANCE);
        let vehicle_len = front - back;
        self.members
            .insert(idx, Queued::StaticBlockage { cause, front, back });
        self.reserved_length += vehicle_len + FOLLOWING_DISTANCE;
    }

    /// Record that a car is no longer blocking a static portion of the queue.
    pub fn clear_static_blockage(&mut self, caused_by: CarID, idx: usize) {
        let blockage = self.members.remove(idx).unwrap();
        match blockage {
            Queued::StaticBlockage { front, back, cause } => {
                assert_eq!(caused_by, cause);
                let vehicle_len = front - back;
                self.reserved_length -= vehicle_len + FOLLOWING_DISTANCE;
            }
            _ => unreachable!(),
        }
    }

    /// Record that a car is starting to change lanes away from this queue.
    pub fn replace_car_with_dynamic_blockage(&mut self, car: &Car, idx: usize) {
        self.remove_car_from_idx(car.vehicle.id, idx);
        self.members.insert(
            idx,
            Queued::DynamicBlockage {
                cause: car.vehicle.id,
                vehicle_len: car.vehicle.length,
            },
        );
        // We don't need to touch reserved_length -- it's still vehicle_len + FOLLOWING_DISTANCE
    }

    /// Record that a car is no longer blocking a dynamic portion of the queue.
    pub fn clear_dynamic_blockage(&mut self, caused_by: CarID, idx: usize) {
        let blockage = self.members.remove(idx).unwrap();
        match blockage {
            Queued::DynamicBlockage { cause, vehicle_len } => {
                assert_eq!(caused_by, cause);
                self.reserved_length -= vehicle_len + FOLLOWING_DISTANCE;
            }
            _ => unreachable!(),
        }
    }

    /// True if a static blockage can be inserted into the queue without anything already there
    /// intersecting it. Returns the index if so. The position represents the front of the
    /// blockage.
    pub fn can_block_from_driveway(
        &self,
        pos: &Position,
        vehicle_len: Distance,
        now: Time,
        cars: &FixedMap<CarID, Car>,
        queues: &HashMap<Traversable, Queue>,
    ) -> Option<usize> {
        self.get_idx_to_insert_car(pos.dist_along(), vehicle_len, now, cars, queues)
    }

    /// Get all cars in the queue, not including the laggy head or blockages.
    ///
    /// TODO Do NOT use this for calculating indices or getting the leader/follower. Might be safer
    /// to just hide this and only expose number of active cars, first, and last.
    pub fn get_active_cars(&self) -> Vec<CarID> {
        self.members
            .iter()
            .filter_map(|x| match x {
                Queued::Vehicle(c) => Some(*c),
                Queued::StaticBlockage { .. } => None,
                Queued::DynamicBlockage { .. } => None,
            })
            .collect()
    }

    /// Remove a car from a position. Need to separately do free_reserved_space.
    pub fn remove_car_from_idx(&mut self, car: CarID, idx: usize) {
        assert_eq!(self.members.remove(idx), Some(Queued::Vehicle(car)));
    }

    /// If a car thinks it's reached the end of the queue, double check. Blockages or laggy heads
    /// might be in the way.
    pub fn is_car_at_front(&self, car: CarID) -> bool {
        self.laggy_head.is_none() && self.members.get(0) == Some(&Queued::Vehicle(car))
    }
}

fn validate_positions(
    dists: &[QueueEntry],
    cars: &FixedMap<CarID, Car>,
    now: Time,
    id: Traversable,
) {
    for pair in dists.windows(2) {
        if pair[0].back - FOLLOWING_DISTANCE < pair[1].front {
            dump_cars(dists, cars, id, now);
            panic!(
                "get_car_positions wound up with bad positioning: {} then {}\n{:?}",
                pair[0].front, pair[1].front, dists
            );
        }
    }
}

fn dump_cars(dists: &[QueueEntry], cars: &FixedMap<CarID, Car>, id: Traversable, now: Time) {
    println!("\nOn {} at {}...", id, now);
    for entry in dists {
        println!("- {:?} @ {}..{}", entry.member, entry.front, entry.back);
        match entry.member {
            Queued::Vehicle(id) => match cars[&id].state {
                CarState::Crossing(ref time_int, ref dist_int) => {
                    println!(
                        "  Going {} .. {} during {} .. {}",
                        dist_int.start, dist_int.end, time_int.start, time_int.end
                    );
                }
                CarState::ChangingLanes {
                    ref new_time,
                    ref new_dist,
                    ..
                } => {
                    println!(
                        "  Going {} .. {} during {} .. {}, also in the middle of lane-changing",
                        new_dist.start, new_dist.end, new_time.start, new_time.end
                    );
                }
                CarState::Queued { .. } => {
                    println!("  Queued currently");
                }
                CarState::WaitingToAdvance { .. } => {
                    println!("  WaitingToAdvance currently");
                }
                CarState::Unparking { ref time_int, .. } => {
                    println!("  Unparking during {} .. {}", time_int.start, time_int.end);
                }
                CarState::Parking(_, _, ref time_int) => {
                    println!("  Parking during {} .. {}", time_int.start, time_int.end);
                }
                CarState::IdlingAtStop(_, ref time_int) => {
                    println!("  Idling during {} .. {}", time_int.start, time_int.end);
                }
            },
            Queued::StaticBlockage { cause, .. } => {
                println!("  Static blockage by {}", cause);
            }
            Queued::DynamicBlockage { cause, vehicle_len } => {
                println!("  Dynamic blockage of length {} by {}", vehicle_len, cause);
            }
        }
    }
    println!();
}
