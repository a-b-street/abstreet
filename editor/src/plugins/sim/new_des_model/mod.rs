mod car;
mod intersection;
mod queue;

pub use self::car::{Car, CarState, DistanceInterval, TimeInterval};
use self::intersection::IntersectionController;
use self::queue::Queue;
use ezgui::{Color, GfxCtx};
use geom::{Distance, Duration, Speed};
use map_model::{IntersectionID, LaneID, Map, Traversable, TurnID, LANE_THICKNESS};
use sim::{CarID, DrawCarInput};
use std::collections::{BTreeMap, VecDeque};

pub const VEHICLE_LENGTH: Distance = Distance::const_meters(4.0);
pub const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);
pub const FREEFLOW: Color = Color::CYAN;
pub const WAITING: Color = Color::RED;

pub struct World {
    queues: BTreeMap<LaneID, Queue>,
    intersections: BTreeMap<IntersectionID, IntersectionController>,

    spawn_later: Vec<(CarID, Option<Speed>, Vec<Traversable>, Duration, Distance)>,
}

impl World {
    pub fn new(map: &Map) -> World {
        let mut world = World {
            queues: BTreeMap::new(),
            intersections: BTreeMap::new(),
            spawn_later: Vec::new(),
        };

        for l in map.all_lanes() {
            if l.is_for_moving_vehicles() {
                world.queues.insert(
                    l.id,
                    Queue {
                        id: l.id,
                        cars: VecDeque::new(),
                        max_capacity: ((l.length() / (VEHICLE_LENGTH + FOLLOWING_DISTANCE)).floor()
                            as usize)
                            .max(1),
                        lane_len: l.length(),
                    },
                );
            }
        }

        for i in map.all_intersections() {
            world.intersections.insert(
                i.id,
                IntersectionController {
                    id: i.id,
                    accepted: None,
                },
            );
        }

        world
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
                    CarState::CrossingLane(_, _) => {
                        num_freeflow += 1;
                    }
                    CarState::Queued => {
                        num_waiting += 1;
                    }
                    CarState::CrossingTurn(_) => unreachable!(),
                };
            }

            let l = map.get_l(queue.id);

            if num_waiting > 0 {
                // Short lanes exist
                let start = (l.length()
                    - f64::from(num_waiting) * (VEHICLE_LENGTH + FOLLOWING_DISTANCE))
                    .max(Distance::ZERO);
                g.draw_polygon(
                    WAITING,
                    &l.lane_center_pts
                        .slice(start, l.length())
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
            if num_freeflow > 0 {
                g.draw_polygon(
                    FREEFLOW,
                    &l.lane_center_pts
                        .slice(
                            Distance::ZERO,
                            f64::from(num_freeflow) * (VEHICLE_LENGTH + FOLLOWING_DISTANCE),
                        )
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
        }

        for i in self.intersections.values() {
            if let Some(ref car) = i.accepted {
                g.draw_polygon(
                    FREEFLOW,
                    &map.get_t(car.path[0].as_turn())
                        .geom
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
                    .map(|(car, dist)| car.get_draw_car(dist, map)),
            );
        }
        for i in self.intersections.values() {
            result.extend(i.get_draw_cars(time, map));
        }
        result
    }

    pub fn get_draw_cars_on(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawCarInput> {
        match on {
            Traversable::Lane(l) => match self.queues.get(&l) {
                Some(q) => q
                    .get_car_positions(time)
                    .into_iter()
                    .map(|(car, dist)| car.get_draw_car(dist, map))
                    .collect(),
                None => Vec::new(),
            },
            Traversable::Turn(t) => self.intersections[&t.parent]
                .get_draw_cars(time, map)
                .into_iter()
                .filter(|d| d.on == Traversable::Turn(t))
                .collect(),
        }
    }

    pub fn spawn_car(
        &mut self,
        id: CarID,
        max_speed: Option<Speed>,
        path: Vec<Traversable>,
        start_time: Duration,
        start_dist: Distance,
        map: &Map,
    ) {
        if start_dist < VEHICLE_LENGTH {
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

        self.spawn_later
            .push((id, max_speed, path, start_time, start_dist));
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map) {
        // Promote CrossingLane to Queued.
        for queue in self.queues.values_mut() {
            for car in queue.cars.iter_mut() {
                if let CarState::CrossingLane(ref time_int, _) = car.state {
                    if time > time_int.end {
                        car.state = CarState::Queued;
                    }
                }
            }
        }

        // Delete head cars that're completely done.
        for queue in self.queues.values_mut() {
            while !queue.is_empty() {
                if let CarState::Queued = queue.cars[0].state {
                    if queue.cars[0].path.len() == 1 {
                        queue.cars.pop_front();
                        // TODO Should have some brief delay to creep forwards VEHICLE_LENGTH +
                        // FOLLOWING_DISTANCE.
                        continue;
                    }
                }
                break;
            }
        }

        // Figure out where everybody wants to go next.
        // (head of this lane ready to go, what they want next)
        let mut cars_ready_to_turn: Vec<(LaneID, TurnID)> = Vec::new();
        for queue in self.queues.values() {
            if queue.is_empty() {
                continue;
            }
            let car = &queue.cars[0];
            if let CarState::Queued = car.state {
                cars_ready_to_turn.push((queue.id, car.path[1].as_turn()));
            }
        }

        // Lane->Turn transitions
        for (from, turn) in cars_ready_to_turn {
            let i = turn.parent;
            if self.intersections[&i].accepted.is_some() {
                continue;
            }
            if self.queues[&turn.dst].is_full() {
                continue;
            }

            let mut car = self
                .queues
                .get_mut(&from)
                .unwrap()
                .cars
                .pop_front()
                .unwrap();
            car.last_steps.push_front(car.path.pop_front().unwrap());
            car.trim_last_steps(map);
            car.state = CarState::CrossingTurn(TimeInterval {
                start: time,
                end: time
                    + time_to_cross(
                        &DistanceInterval {
                            start: Distance::ZERO,
                            end: map.get_t(turn).geom.length(),
                        },
                        Traversable::Turn(turn).speed_limit(map),
                        car.max_speed,
                    ),
            });
            self.intersections.get_mut(&i).unwrap().accepted = Some(car);
        }

        // Turn->Lane transitions
        for i in self.intersections.values_mut() {
            if i.accepted.is_none() {
                continue;
            }
            let end_time = match i.accepted.as_ref().unwrap().state {
                CarState::CrossingTurn(ref int) => int.end,
                _ => unreachable!(),
            };
            if time < end_time {
                continue;
            }

            let mut car = i.accepted.take().unwrap();
            car.last_steps.push_front(car.path.pop_front().unwrap());
            car.trim_last_steps(map);
            let lane = car.path[0].as_lane();
            if self.queues[&lane].is_full() {
                panic!(
                    "{} is full -- has {:?} at {} -- but {} just finished a turn at {}",
                    lane,
                    self.queues[&lane]
                        .cars
                        .iter()
                        .map(|car| car.id)
                        .collect::<Vec<CarID>>(),
                    time,
                    car.id,
                    i.id
                );
            }
            let dist_int = DistanceInterval {
                start: Distance::ZERO,
                end: map.get_l(lane).length(),
            };
            let dt = time_to_cross(
                &dist_int,
                map.get_parent(lane).get_speed_limit(),
                car.max_speed,
            );
            car.state = CarState::CrossingLane(
                TimeInterval {
                    start: time,
                    end: time + dt,
                },
                dist_int,
            );
            self.queues.get_mut(&lane).unwrap().cars.push_back(car);
        }

        // Spawn cars at the end, so we can see the correct state of everything else at this time.
        let mut retain_spawn = Vec::new();
        for (id, max_speed, path, start_time, start_dist) in self.spawn_later.drain(..) {
            let mut spawned = false;
            let first_lane = path[0].as_lane();

            if time >= start_time
                && self.intersections[&map.get_l(first_lane).src_i]
                    .accepted
                    .as_ref()
                    .map(|car| car.path[1].as_lane() != first_lane)
                    .unwrap_or(true)
            {
                if let Some(idx) = self.queues[&first_lane].get_idx_to_insert_car(start_dist, time)
                {
                    let dist_int = DistanceInterval {
                        start: start_dist,
                        end: map.get_l(first_lane).length(),
                    };
                    let dt = time_to_cross(
                        &dist_int,
                        map.get_parent(first_lane).get_speed_limit(),
                        max_speed,
                    );
                    self.queues.get_mut(&first_lane).unwrap().cars.insert(
                        idx,
                        Car {
                            id,
                            max_speed,
                            path: VecDeque::from(path.clone()),
                            state: CarState::CrossingLane(
                                TimeInterval {
                                    start: time,
                                    end: time + dt,
                                },
                                dist_int,
                            ),
                            last_steps: VecDeque::new(),
                        },
                    );
                    spawned = true;
                    //println!("{} spawned at {}", id, time);
                }
            }
            if !spawned {
                retain_spawn.push((id, max_speed, path, start_time, start_dist));
            }
        }
        self.spawn_later = retain_spawn;
    }
}

fn time_to_cross(
    dist_int: &DistanceInterval,
    speed_limit: Speed,
    max_speed: Option<Speed>,
) -> Duration {
    let mut speed = speed_limit;
    if let Some(s) = max_speed {
        speed = speed.min(s);
    }
    (dist_int.end - dist_int.start) / speed
}
