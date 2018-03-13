// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ezgui::canvas;
use ezgui::canvas::GfxCtx;
use graphics;
use map_model::{Map, Pt2D, RoadID, TurnID};
use multimap::MultiMap;
use ordered_float::NotNaN;
use rand::{thread_rng, Rng};
use render::DrawMap;

const SPEED_LIMIT_METERS_PER_SECOND: f64 = 8.9408;
const FOLLOWING_DISTANCE_METERS: f64 = 8.0;

const CAR_WIDTH: f64 = 2.0;
const CAR_LENGTH: f64 = 4.5;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CarID(pub usize);

// TODO move this out
pub struct DrawCar {
    id: CarID,
    front: Pt2D,
    // TODO type this
    angle_rads: f64,
}

impl DrawCar {
    fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        let line = graphics::Line::new_round(color, CAR_WIDTH / 2.0);
        line.draw(
            [
                self.front.x(),
                self.front.y(),
                self.front.x() - CAR_LENGTH * self.angle_rads.cos(),
                self.front.y() - CAR_LENGTH * self.angle_rads.sin(),
            ],
            &g.ctx.draw_state,
            g.ctx.transform,
            g.gfx,
        );
    }
}

// TODO this name isn't quite right :)
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum On {
    Road(RoadID),
    Turn(TurnID),
}

impl On {
    fn as_road(&self) -> RoadID {
        match self {
            &On::Road(id) => id,
            &On::Turn(_) => panic!("not a road"),
        }
    }

    fn as_turn(&self) -> TurnID {
        match self {
            &On::Turn(id) => id,
            &On::Road(_) => panic!("not a turn"),
        }
    }

    fn length(&self, draw_map: &DrawMap) -> f64 {
        match self {
            &On::Road(id) => draw_map.get_r(id).length(),
            &On::Turn(id) => draw_map.get_t(id).length(),
        }
    }

    // TODO and angle!
    fn dist_along(&self, dist: f64, draw_map: &DrawMap) -> (Pt2D, f64) {
        match self {
            &On::Road(id) => draw_map.get_r(id).dist_along(dist),
            &On::Turn(id) => draw_map.get_t(id).dist_along(dist),
        }
    }
}

#[derive(Clone)]
struct Car {
    id: CarID,
    on: On,
    // When did the car start the current On?
    started_at: f64,
    // TODO is this only valid once started_at is high enough?
    waiting_for: Option<On>,
}

impl Car {
    // TODO ideally this could consume, but having trouble moving out of vec
    fn step(&self, time: f64, draw_map: &DrawMap, map: &Map, sim: &Sim) -> Option<Car> {
        if let Some(on) = self.waiting_for {
            if sim.has_room_now(on) {
                let mut copy = self.clone();
                copy.on = on;
                return Some(copy);
            } else {
                return Some(self.clone());
            }
        }

        let dist = SPEED_LIMIT_METERS_PER_SECOND * (time - self.started_at);
        if dist >= self.on.length(draw_map) {
            let mut copy = self.clone();
            // TODO could calculate leftover (and deal with large timesteps, small
            // roads)
            copy.started_at = time;
            match self.on {
                // For now, just kill off cars that're stuck on disconnected bits of the map
                On::Road(id) if map.get_turns_from_road(id).is_empty() => None,
                On::Road(id) => {
                    // TODO need to make sure the turn is clear too, once we start listening to
                    // intersection policies
                    let t = On::Turn(self.choose_turn(id, map));
                    if sim.has_room_now(t) {
                        copy.on = t;
                    } else {
                        copy.waiting_for = Some(t);
                    }
                    Some(copy)
                }
                On::Turn(id) => {
                    let r = On::Road(map.get_t(id).dst);
                    if sim.has_room_now(r) {
                        copy.on = r;
                    } else {
                        copy.waiting_for = Some(r);
                    }
                    Some(copy)
                }
            }
        } else {
            Some(self.clone())
        }
    }

    fn choose_turn(&self, from: RoadID, map: &Map) -> TurnID {
        assert!(self.waiting_for.is_none());
        let mut rng = thread_rng();
        rng.choose(&map.get_turns_from_road(from)).unwrap().id
    }

    // Returns the angle and the dist along the road/turn too
    fn get_best_case_pos(&self, time: f64, draw_map: &DrawMap) -> ((Pt2D, f64), f64) {
        let dist = SPEED_LIMIT_METERS_PER_SECOND * (time - self.started_at);
        (self.get_pos_for_dist(dist, draw_map), dist)
    }

    // and angle
    fn get_pos_for_dist(&self, dist: f64, draw_map: &DrawMap) -> (Pt2D, f64) {
        self.on.dist_along(dist, draw_map)
    }
}

struct SimQueue {
    id: On,
    cars_queue: Vec<CarID>,
    // TODO it'd be neat if different aspects of a road could contribute parts of tooltips, like
    // this
    capacity: usize,
}

impl SimQueue {
    fn new(id: On, draw_map: &DrawMap) -> SimQueue {
        let capacity = (id.length(draw_map) / FOLLOWING_DISTANCE_METERS).floor() as usize;
        SimQueue {
            id,
            capacity: if capacity == 0 { 1 } else { capacity },
            cars_queue: Vec::new(),
        }
    }

    fn join_at_end(&mut self, car: CarID) {
        assert!(self.cars_queue.len() < self.capacity);
        // TODO safety checks needed
        self.cars_queue.push(car);
    }

    fn reset(&mut self, ids: &Vec<CarID>, cars: &Vec<Option<Car>>) {
        assert!(cars.len() <= self.capacity);
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        self.cars_queue
            .sort_by_key(|id| NotNaN::new(cars[id.0].as_ref().unwrap().started_at).unwrap());
    }

    fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    fn get_draw_cars(&self, sim: &Sim, draw_map: &DrawMap) -> Vec<DrawCar> {
        if self.cars_queue.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let ((pos1, angle1), dist_along1) = sim.cars[self.cars_queue[0].0]
            .as_ref()
            .unwrap()
            .get_best_case_pos(sim.time, draw_map);
        results.push(DrawCar {
            id: self.cars_queue[0],
            front: pos1,
            angle_rads: angle1,
        });
        let mut dist_along_bound = dist_along1;

        // TODO handle when we go negative here. shouldn't even be able to join that road/turn yet.
        for id in self.cars_queue.iter().skip(1) {
            let c = &sim.cars[id.0].as_ref().unwrap();
            let ((pos, angle), dist_along) = c.get_best_case_pos(sim.time, draw_map);
            if dist_along_bound - FOLLOWING_DISTANCE_METERS > dist_along {
                results.push(DrawCar {
                    id: *id,
                    front: pos,
                    angle_rads: angle,
                });
                dist_along_bound = dist_along;
            } else {
                dist_along_bound -= FOLLOWING_DISTANCE_METERS;
                // If not, we disobeyed capacity and let too many cars on
                assert!(dist_along_bound >= 0.0);
                let (pt, angle) = c.get_pos_for_dist(dist_along_bound, draw_map);
                results.push(DrawCar {
                    id: *id,
                    front: pt,
                    angle_rads: angle,
                });
            }
        }

        results
    }
}

pub struct Sim {
    // TODO until I figure out the CarID stuff, just keep around ghosts of cars
    cars: Vec<Option<Car>>,
    roads: Vec<SimQueue>,
    turns: Vec<SimQueue>,
    time: f64,
}

impl Sim {
    pub fn new(map: &Map, draw_map: &DrawMap) -> Sim {
        Sim {
            cars: Vec::new(),
            roads: map.all_roads()
                .iter()
                .map(|r| SimQueue::new(On::Road(r.id), draw_map))
                .collect(),
            turns: map.all_turns()
                .iter()
                .map(|t| SimQueue::new(On::Turn(t.id), draw_map))
                .collect(),
            time: 0.0,
        }
    }

    pub fn spawn_one_on_road(&mut self, road: RoadID) {
        let id = CarID(self.cars.len());
        self.cars.push(Some(Car {
            id,
            started_at: self.time,
            on: On::Road(road),
            waiting_for: None,
        }));
        self.roads[road.0].join_at_end(id);
    }

    pub fn spawn_many_on_empty_roads(&mut self, num_cars: usize) {
        let mut roads: Vec<RoadID> = self.roads
            .iter()
            .filter_map(|r| {
                if r.is_empty() {
                    Some(r.id.as_road())
                } else {
                    None
                }
            })
            .collect();
        let mut rng = thread_rng();
        rng.shuffle(&mut roads);

        let n = num_cars.min(roads.len());
        for i in 0..n {
            self.spawn_one_on_road(roads[i]);
        }
        println!("Spawned {}", n);
    }

    pub fn step(&mut self, dt_s: f64, draw_map: &DrawMap, map: &Map) {
        self.time += dt_s;

        // Can choose actions in any order
        // TODO cant quite get this written well
        let mut new_cars = Vec::new();
        for i in 0..self.cars.len() {
            if self.cars[i].is_some() {
                new_cars.push(
                    self.cars[i]
                        .as_ref()
                        .unwrap()
                        .step(self.time, draw_map, map, &self),
                );
            } else {
                new_cars.push(None);
            }
        }
        self.cars = new_cars;

        // Group cars by road and turn
        let mut cars_per_road = MultiMap::new();
        let mut cars_per_turn = MultiMap::new();
        for maybe_car in &self.cars {
            if let &Some(ref c) = maybe_car {
                match c.on {
                    On::Road(id) => cars_per_road.insert(id, c.id),
                    On::Turn(id) => cars_per_turn.insert(id, c.id),
                };
            }
        }

        // Reset all queues
        for r in &mut self.roads {
            if let Some(v) = cars_per_road.get_vec(&r.id.as_road()) {
                r.reset(v, &self.cars);
            } else {
                r.reset(&Vec::new(), &self.cars);
            }
            //r.reset(cars_per_road.get_vec(&r.id).unwrap_or_else(|| &Vec::new()), &self.cars);
        }
        for t in &mut self.turns {
            if let Some(v) = cars_per_turn.get_vec(&t.id.as_turn()) {
                t.reset(v, &self.cars);
            } else {
                t.reset(&Vec::new(), &self.cars);
            }
        }
    }

    pub fn draw_cars_on_road(&self, r: RoadID, draw_map: &DrawMap, g: &mut GfxCtx) {
        for c in &self.roads[r.0].get_draw_cars(&self, draw_map) {
            let color = if self.cars[c.id.0].as_ref().unwrap().waiting_for.is_none() {
                canvas::CYAN
            } else {
                canvas::RED
            };
            c.draw(g, color);
        }
    }

    pub fn draw_cars_on_turn(&self, t: TurnID, draw_map: &DrawMap, g: &mut GfxCtx) {
        for c in &self.turns[t.0].get_draw_cars(&self, draw_map) {
            c.draw(g, canvas::CYAN);
        }
    }

    pub fn draw(&self, canvas: &canvas::Canvas, g: &mut GfxCtx) {
        let mut count = 0;
        for c in &self.cars {
            if c.is_some() {
                count += 1;
            }
        }
        canvas.draw_osd_notification(
            g,
            &vec![format!("Time: {0:.2}s, {1} live cars", self.time, count)],
        );
    }

    // TODO this wont prevent several cars simultaneously deciding to clash and enter a road and
    // fill it up too much
    fn has_room_now(&self, on: On) -> bool {
        match on {
            On::Road(id) => self.roads[id.0].cars_queue.len() < self.roads[id.0].capacity,
            On::Turn(id) => self.turns[id.0].cars_queue.len() < self.turns[id.0].capacity,
        }
    }
}
