use crate::plugins::sim::new_des_model::{
    Car, CarState, IntersectionController, ParkedCar, ParkingSimState, Queue, TimeInterval,
    Vehicle, FOLLOWING_DISTANCE, MAX_VEHICLE_LENGTH,
};
use ezgui::{Color, GfxCtx};
use geom::{Distance, Duration};
use map_model::{IntersectionID, Map, Traversable, LANE_THICKNESS};
use sim::DrawCarInput;
use std::collections::{BTreeMap, VecDeque};

const FREEFLOW: Color = Color::CYAN;
const WAITING: Color = Color::RED;

const TIME_TO_UNPARK: Duration = Duration::const_seconds(10.0);

pub struct DrivingSimState {
    queues: BTreeMap<Traversable, Queue>,
    intersections: BTreeMap<IntersectionID, IntersectionController>,

    spawn_later: Vec<(
        Vehicle,
        Vec<Traversable>,
        Duration,
        Distance,
        Distance,
        Option<ParkedCar>,
    )>,
}

impl DrivingSimState {
    pub fn new(map: &Map) -> DrivingSimState {
        let mut sim = DrivingSimState {
            queues: BTreeMap::new(),
            intersections: BTreeMap::new(),
            spawn_later: Vec::new(),
        };

        for l in map.all_lanes() {
            if l.is_for_moving_vehicles() {
                let q = Queue::new(Traversable::Lane(l.id), map);
                sim.queues.insert(q.id, q);
            }
        }
        for t in map.all_turns().values() {
            if !t.between_sidewalks() {
                let q = Queue::new(Traversable::Turn(t.id), map);
                sim.queues.insert(q.id, q);
            }
        }

        for i in map.all_intersections() {
            sim.intersections
                .insert(i.id, IntersectionController::new(i.id));
        }

        sim
    }

    pub fn draw_unzoomed(&self, _time: Duration, g: &mut GfxCtx, map: &Map) {
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            let mut num_waiting = 0;
            let mut num_freeflow = 0;
            for car in &queue.cars {
                match car.state {
                    CarState::Crossing(_, _) | CarState::Unparking(_, _) => {
                        num_freeflow += 1;
                    }
                    CarState::Queued => {
                        num_waiting += 1;
                    }
                };
            }

            if num_waiting > 0 {
                // Short lanes/turns exist
                let start = (queue.geom_len
                    - f64::from(num_waiting) * (MAX_VEHICLE_LENGTH + FOLLOWING_DISTANCE))
                    .max(Distance::ZERO);
                g.draw_polygon(
                    WAITING,
                    &queue
                        .id
                        .slice(start, queue.geom_len, map)
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
            if num_freeflow > 0 {
                g.draw_polygon(
                    FREEFLOW,
                    &queue
                        .id
                        .slice(
                            Distance::ZERO,
                            f64::from(num_freeflow) * (MAX_VEHICLE_LENGTH + FOLLOWING_DISTANCE),
                            map,
                        )
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
        }
    }

    pub fn get_all_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut result = Vec::new();
        for queue in self.queues.values() {
            result.extend(
                queue
                    .get_car_positions(time)
                    .into_iter()
                    .map(|(car, dist)| car.get_draw_car(dist, time, map)),
            );
        }
        result
    }

    pub fn get_draw_cars_on(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawCarInput> {
        match self.queues.get(&on) {
            Some(q) => q
                .get_car_positions(time)
                .into_iter()
                .map(|(car, dist)| car.get_draw_car(dist, time, map))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn spawn_car(
        &mut self,
        vehicle: Vehicle,
        path: Vec<Traversable>,
        start_time: Duration,
        start_dist: Distance,
        end_dist: Distance,
        maybe_parked_car: Option<ParkedCar>,
        map: &Map,
        parking: &ParkingSimState,
    ) {
        if let Some(ref parked_car) = maybe_parked_car {
            assert_eq!(parked_car.vehicle, vehicle);
            assert_eq!(
                start_dist,
                parking
                    .spot_to_driving_pos(parked_car.spot, &vehicle, path[0].as_lane(), map,)
                    .dist_along()
            );
        }

        if start_dist < vehicle.length {
            panic!(
                "Can't spawn a car at {}; too close to the start",
                start_dist
            );
        }
        if start_dist >= path[0].length(map) {
            panic!(
                "Can't spawn a car at {}; {:?} isn't that long",
                start_dist, path[0]
            );
        }
        if end_dist >= path.last().unwrap().length(map) {
            panic!(
                "Can't end a car at {}; {:?} isn't that long",
                end_dist,
                path.last().unwrap()
            );
        }
        if path.len() == 1 && start_dist >= end_dist {
            panic!(
                "Can't start a car with one path in its step and go from {} to {}",
                start_dist, end_dist
            );
        }

        self.spawn_later.push((
            vehicle,
            path,
            start_time,
            start_dist,
            end_dist,
            maybe_parked_car,
        ));
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map, parking: &mut ParkingSimState) {
        // Promote Crossing to Queued and Unparking to Crossing.
        for queue in self.queues.values_mut() {
            for car in queue.cars.iter_mut() {
                if let CarState::Crossing(ref time_int, _) = car.state {
                    if time > time_int.end {
                        car.state = CarState::Queued;
                    }
                } else if let CarState::Unparking(front, ref time_int) = car.state {
                    if time > time_int.end {
                        car.state = car.crossing_state(front, time, map);
                    }
                }
            }
        }

        // Delete cars that're completely done. These might not necessarily be the queue head,
        // since cars can stop early.
        for queue in self.queues.values_mut() {
            queue.cars.retain(|car| match car.state {
                CarState::Queued => car.path.len() > 1,
                _ => true,
            });
        }
        // TODO Need to update the Queued followers behind those that just vanished, to avoid
        // jumps.

        // Figure out where everybody wants to go next.
        let mut head_cars_ready_to_advance: Vec<Traversable> = Vec::new();
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            let car = &queue.cars[0];
            if let CarState::Queued = car.state {
                head_cars_ready_to_advance.push(queue.id);
            }
        }

        // Carry out the transitions.
        for from in head_cars_ready_to_advance {
            let car_id = self.queues[&from].cars[0].vehicle.id;
            let goto = self.queues[&from].cars[0].path[1];

            // Always need to do this check.
            if !self.queues[&goto].room_at_end(time) {
                continue;
            }

            if let Traversable::Turn(t) = goto {
                if !self
                    .intersections
                    .get_mut(&t.parent)
                    .unwrap()
                    .maybe_start_turn(car_id, t, &self.queues, time, map)
                {
                    continue;
                }
            }

            let mut car = self
                .queues
                .get_mut(&from)
                .unwrap()
                .cars
                .pop_front()
                .unwrap();

            // Update the follower so that they don't suddenly jump forwards.
            if let Some(ref mut follower) = self.queues.get_mut(&from).unwrap().cars.front_mut() {
                // TODO This still jumps a bit -- the lead car's back is still sticking out. Need
                // to still be bound by them.
                match follower.state {
                    CarState::Queued => {
                        follower.state = follower.crossing_state(
                            // Since the follower was Queued, this must be where they are
                            from.length(map) - car.vehicle.length - FOLLOWING_DISTANCE,
                            time,
                            map,
                        );
                    }
                    // They weren't blocked
                    CarState::Crossing(_, _) | CarState::Unparking(_, _) => {}
                }
            }

            let last_step = car.path.pop_front().unwrap();
            car.last_steps.push_front(last_step);
            car.trim_last_steps(map);
            car.state = car.crossing_state(Distance::ZERO, time, map);

            if goto.maybe_lane().is_some() {
                // TODO Actually, don't call turn_finished until the car is at least vehicle.length
                // + FOLLOWING_DISTANCE into the next lane. This'll be hard to predict when we're
                // event-based, so hold off on this bit of realism.
                self.intersections
                    .get_mut(&last_step.as_turn().parent)
                    .unwrap()
                    .turn_finished(car.vehicle.id, last_step.as_turn());
            }

            self.queues.get_mut(&goto).unwrap().cars.push_back(car);
        }

        // Spawn cars at the end, so we can see the correct state of everything else at this time.
        let mut retain_spawn = Vec::new();
        for (vehicle, path, start_time, start_dist, end_dist, maybe_parked_car) in
            self.spawn_later.drain(..)
        {
            let mut spawned = false;
            let first_lane = path[0].as_lane();

            if time >= start_time
                && self.intersections[&map.get_l(first_lane).src_i]
                    .nobody_headed_towards(first_lane)
            {
                if let Some(idx) = self.queues[&Traversable::Lane(first_lane)]
                    .get_idx_to_insert_car(start_dist, vehicle.length, time)
                {
                    let mut car = Car {
                        vehicle: vehicle.clone(),
                        path: VecDeque::from(path.clone()),
                        end_dist,
                        state: CarState::Queued,
                        last_steps: VecDeque::new(),
                    };
                    if maybe_parked_car.is_some() {
                        car.state = CarState::Unparking(
                            start_dist,
                            TimeInterval::new(time, time + TIME_TO_UNPARK),
                        );
                    } else {
                        car.state = car.crossing_state(start_dist, time, map);
                    }
                    self.queues
                        .get_mut(&Traversable::Lane(first_lane))
                        .unwrap()
                        .cars
                        .insert(idx, car);
                    spawned = true;
                    //println!("{} spawned at {}", vehicle.id, time);
                }
            }
            if spawned {
                if let Some(parked_car) = maybe_parked_car {
                    parking.remove_parked_car(parked_car);
                }
            } else {
                retain_spawn.push((
                    vehicle,
                    path,
                    start_time,
                    start_dist,
                    end_dist,
                    maybe_parked_car,
                ));
            }
        }
        self.spawn_later = retain_spawn;
    }
}
