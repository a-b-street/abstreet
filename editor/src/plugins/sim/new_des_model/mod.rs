use ezgui::{Color, GfxCtx};
use geom::{Distance, Duration, PolyLine, Speed};
use map_model::{IntersectionID, LaneID, Map, Traversable, TurnID, LANE_THICKNESS};
use sim::CarID;
use std::collections::{BTreeMap, VecDeque};

const VEHICLE_LENGTH: Distance = Distance::const_meters(4.0);
const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);
const FREEFLOW: Color = Color::CYAN;
const WAITING: Color = Color::RED;

pub struct World {
    queues: BTreeMap<LaneID, Queue>,
    intersections: BTreeMap<IntersectionID, IntersectionController>,

    spawn_later: Vec<(CarID, Option<Speed>, Vec<Traversable>, Duration)>,
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
                    },
                );
            }
        }

        for i in map.all_intersections() {
            world.intersections.insert(
                i.id,
                IntersectionController {
                    _id: i.id,
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
                    CarState::CrossingLane(_) => {
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
                g.draw_polygon(
                    WAITING,
                    &l.lane_center_pts
                        .slice(
                            l.length()
                                - (num_waiting as f64) * (VEHICLE_LENGTH + FOLLOWING_DISTANCE),
                            l.length(),
                        )
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
                            (num_freeflow as f64) * (VEHICLE_LENGTH + FOLLOWING_DISTANCE),
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

    pub fn draw_detailed(&self, time: Duration, g: &mut GfxCtx, map: &Map) {
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            let l = map.get_l(queue.id);

            let mut last_car_back: Option<Distance> = None;

            for car in &queue.cars {
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
                            .unwrap_or(l.length());
                        (i.percent(time) * bound, FREEFLOW)
                    }
                    CarState::CrossingTurn(_) => unreachable!(),
                };

                // There's backfill and a car shouldn't have been able to enter yet, but it's a
                // temporary condition -- there's enough rest capacity.
                if front < Distance::ZERO {
                    println!(
                        "Queue temporarily backed up on {} -- can't draw {}",
                        queue.id, car.id
                    );
                    break;
                }

                car.draw(front, color, g, map);
                last_car_back = Some(front - VEHICLE_LENGTH);
            }
        }

        for i in self.intersections.values() {
            if let Some(ref car) = i.accepted {
                let t = map.get_t(car.path[0].as_turn());
                let percent = match car.state {
                    CarState::CrossingTurn(ref int) => int.percent(time),
                    _ => unreachable!(),
                };
                car.draw(percent * t.geom.length(), FREEFLOW, g, map);
            }
        }
    }

    pub fn spawn_car(
        &mut self,
        id: CarID,
        max_speed: Option<Speed>,
        path: Vec<Traversable>,
        start_time: Duration,
    ) {
        self.spawn_later.push((id, max_speed, path, start_time));
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map) {
        // Spawn cars.
        let mut retain_spawn = Vec::new();
        for (id, max_speed, path, start_time) in self.spawn_later.drain(..) {
            let first_lane = path[0].as_lane();
            if time >= start_time && !self.queues[&first_lane].is_full() {
                self.queues
                    .get_mut(&first_lane)
                    .unwrap()
                    .cars
                    .push_back(Car {
                        id: id,
                        max_speed,
                        path: VecDeque::from(path.clone()),
                        state: CarState::CrossingLane(TimeInterval {
                            start: time,
                            end: time
                                + time_to_cross(Traversable::Lane(first_lane), map, max_speed),
                        }),
                        last_steps: VecDeque::new(),
                    });
            } else {
                retain_spawn.push((id, max_speed, path, start_time));
            }
        }
        self.spawn_later = retain_spawn;

        // Promote CrossingLane to Queued.
        for queue in self.queues.values_mut() {
            for car in queue.cars.iter_mut() {
                match car.state {
                    CarState::CrossingLane(ref interval) => {
                        if time > interval.end {
                            car.state = CarState::Queued;
                        }
                    }
                    _ => {}
                };
            }
        }

        // Delete head cars that're completely done.
        for queue in self.queues.values_mut() {
            while !queue.is_empty() {
                match queue.cars[0].state {
                    CarState::Queued => {
                        if queue.cars[0].path.len() == 1 {
                            queue.cars.pop_front();
                            // TODO Should have some brief delay to creep forwards VEHICLE_LENGTH +
                            // FOLLOWING_DISTANCE.
                            continue;
                        }
                    }
                    _ => {}
                };
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
            match car.state {
                CarState::Queued => {
                    cars_ready_to_turn.push((queue.id, car.path[1].as_turn()));
                }
                _ => {}
            };
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
                end: time + time_to_cross(Traversable::Turn(turn), map, car.max_speed),
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
            assert!(!self.queues[&lane].is_full());
            car.state = CarState::CrossingLane(TimeInterval {
                start: time,
                end: end_time + time_to_cross(Traversable::Lane(lane), map, car.max_speed),
            });
            self.queues.get_mut(&lane).unwrap().cars.push_back(car);
        }
    }
}

struct TimeInterval {
    start: Duration,
    end: Duration,
}

impl TimeInterval {
    fn percent(&self, t: Duration) -> f64 {
        let x = (t - self.start) / (self.end - self.start);
        assert!(x >= 0.0 && x <= 1.0);
        x
    }
}

struct Queue {
    id: LaneID,
    cars: VecDeque<Car>,
    max_capacity: usize,
}

impl Queue {
    fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }

    fn is_full(&self) -> bool {
        self.cars.len() == self.max_capacity
    }
}

struct IntersectionController {
    _id: IntersectionID,
    accepted: Option<Car>,
}

struct Car {
    id: CarID,
    max_speed: Option<Speed>,
    // Front is always the current step
    path: VecDeque<Traversable>,
    state: CarState,

    // In reverse order -- most recently left is first. The sum length of these must be >=
    // VEHICLE_LENGTH.
    last_steps: VecDeque<Traversable>,
}

impl Car {
    fn trim_last_steps(&mut self, map: &Map) {
        let mut keep = VecDeque::new();
        let mut len = Distance::ZERO;
        for on in self.last_steps.drain(..) {
            len += on.length(map);
            keep.push_back(on);
            if len >= VEHICLE_LENGTH {
                break;
            }
        }
        self.last_steps = keep;
    }

    fn draw(&self, front: Distance, color: Color, g: &mut GfxCtx, map: &Map) {
        assert!(front >= Distance::ZERO);
        if front >= VEHICLE_LENGTH {
            g.draw_polygon(
                color,
                &self.path[0]
                    .slice(front - VEHICLE_LENGTH, front, map)
                    .unwrap()
                    .0
                    .make_polygons(LANE_THICKNESS),
            );
        } else if self.last_steps.is_empty() {
            println!("{} spawned too close", self.id);
        // TODO spawned too close
        } else {
            // TODO This is redoing some of the Path::trace work...
            let mut result = self.path[0]
                .slice(Distance::ZERO, front, map)
                .map(|(pl, _)| pl.points().clone())
                .unwrap_or_else(Vec::new);
            let mut leftover = VEHICLE_LENGTH - front;
            let mut i = 0;
            while leftover > Distance::ZERO {
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

            g.draw_polygon(color, &PolyLine::new(result).make_polygons(LANE_THICKNESS));
        }
    }
}

enum CarState {
    CrossingLane(TimeInterval),
    Queued,
    CrossingTurn(TimeInterval),
}

fn time_to_cross(on: Traversable, map: &Map, max_speed: Option<Speed>) -> Duration {
    let mut speed = on.speed_limit(map);
    if let Some(s) = max_speed {
        speed = speed.min(s);
    }
    on.length(map) / speed
}
