use ezgui::{Color, GfxCtx};
use geom::{Distance, Duration, Speed};
use map_model::{IntersectionID, LaneID, Map, Traversable, TurnID, LANE_THICKNESS};
use sim::CarID;
use std::collections::{BTreeMap, VecDeque};

const VEHICLE_LENGTH: Distance = Distance::const_meters(5.0);
const FREEFLOW: Color = Color::CYAN;
const WAITING: Color = Color::RED;

pub struct World {
    queues: BTreeMap<LaneID, Queue>,
    intersections: BTreeMap<IntersectionID, IntersectionController>,
}

impl World {
    pub fn new(map: &Map) -> World {
        let mut world = World {
            queues: BTreeMap::new(),
            intersections: BTreeMap::new(),
        };

        for l in map.all_lanes() {
            if l.is_for_moving_vehicles() {
                world.queues.insert(
                    l.id,
                    Queue {
                        id: l.id,
                        cars: VecDeque::new(),
                        max_capacity: ((l.length() / VEHICLE_LENGTH).floor() as usize).max(1),
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

    pub fn draw_unzoomed(&self, g: &mut GfxCtx, map: &Map) {
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            let mut num_freeflow = 0;
            let mut num_waiting = 0;
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
            if num_freeflow > 0 {
                g.draw_polygon(
                    FREEFLOW,
                    &l.lane_center_pts
                        .slice(Distance::ZERO, (num_freeflow as f64) * VEHICLE_LENGTH)
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
            if num_waiting > 0 {
                g.draw_polygon(
                    WAITING,
                    &l.lane_center_pts
                        .slice(
                            l.length() - (num_waiting as f64) * VEHICLE_LENGTH,
                            l.length(),
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

    pub fn spawn_car(
        &mut self,
        id: CarID,
        max_speed: Option<Speed>,
        path: Vec<Traversable>,
        map: &Map,
    ) {
        let first_lane = path[0].as_lane();
        let queue = self.queues.get_mut(&first_lane).unwrap();
        assert!(!queue.is_full());

        queue.cars.push_back(Car {
            id,
            max_speed,
            path: VecDeque::from(path),
            state: CarState::CrossingLane(TimeInterval {
                start: Duration::ZERO,
                end: time_to_cross(Traversable::Lane(first_lane), map, max_speed),
            }),
        });
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map) {
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
            if queue.is_empty() {
                continue;
            }
            match queue.cars[0].state {
                CarState::Queued => {
                    if queue.cars[0].path.len() == 1 {
                        queue.cars.pop_front();
                    }
                }
                _ => {}
            };
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
            car.path.pop_front();
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
            car.path.pop_front();
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
    id: IntersectionID,
    accepted: Option<Car>,
}

struct Car {
    id: CarID,
    max_speed: Option<Speed>,
    // Front is always the current step
    path: VecDeque<Traversable>,
    state: CarState,
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
