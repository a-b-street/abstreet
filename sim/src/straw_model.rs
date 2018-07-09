// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use control::ControlMap;
use dimensioned::si;
use draw_car::DrawCar;
use driving::{Action, Car, On, SimQueue};
use intersections::{IntersectionPolicy, StopSign, TrafficSignal};
use map_model;
use map_model::{LaneType, Map, RoadID, TurnID};
use multimap::MultiMap;
use rand::{FromEntropy, Rng, SeedableRng, XorShiftRng};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::f64;
use std::time::{Duration, Instant};
use {CarID, Tick};

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Sim {
    // This is slightly dangerous, but since we'll be using comparisons based on savestating (which
    // captures the RNG), this should be OK for now.
    #[derivative(PartialEq = "ignore")]
    rng: XorShiftRng,
    // TODO investigate slot map-like structures for performance
    // Using BTreeMap instead of HashMap so iteration is deterministic. Should be able to relax
    // this later after step() doesnt need a RNG.
    pub(crate) cars: BTreeMap<CarID, Car>,
    roads: Vec<SimQueue>,
    turns: Vec<SimQueue>,
    intersections: Vec<IntersectionPolicy>,
    pub time: Tick,
    id_counter: usize,
    debug: Option<CarID>,
}

impl Sim {
    pub fn new(map: &Map, rng_seed: Option<u8>) -> Sim {
        let mut rng = XorShiftRng::from_entropy();
        if let Some(seed) = rng_seed {
            rng = XorShiftRng::from_seed([seed; 16]);
        }

        let mut intersections: Vec<IntersectionPolicy> = Vec::new();
        for i in map.all_intersections() {
            if i.has_traffic_signal {
                intersections.push(IntersectionPolicy::TrafficSignalPolicy(TrafficSignal::new(
                    i.id,
                )));
            } else {
                intersections.push(IntersectionPolicy::StopSignPolicy(StopSign::new(i.id)));
            }
        }

        Sim {
            rng,
            intersections,

            cars: BTreeMap::new(),
            // TODO only driving ones
            roads: map.all_roads()
                .iter()
                .map(|r| SimQueue::new(On::Road(r.id), map))
                .collect(),
            turns: map.all_turns()
                .iter()
                .map(|t| SimQueue::new(On::Turn(t.id), map))
                .collect(),
            time: Tick::zero(),
            id_counter: 0,
            debug: None,
        }
    }

    // TODO cars basically start in the intersection, with their front bumper right at the
    // beginning of the road. later, we want cars starting at arbitrary points in the middle of the
    // road (from a building), so just ignore this problem for now.
    pub fn spawn_one_on_road(&mut self, map: &Map, start: RoadID) -> bool {
        if !self.roads[start.0].room_at_end(self.time, &self.cars) {
            return false;
        }
        let id = CarID(self.id_counter);
        self.id_counter += 1;

        let goal = self.rng.choose(map.all_roads()).unwrap();
        if goal.lane_type != LaneType::Driving || goal.id == start {
            println!("Chose bad goal {}", goal.id);
            return false;
        }
        let mut path = if let Some(steps) = map_model::pathfind(map, start, goal.id) {
            VecDeque::from(steps)
        } else {
            println!("No path from {} to {}", start, goal.id);
            return false;
        };
        // path includes the start, but that's not the invariant Car enforces
        path.pop_front();

        self.cars.insert(
            id,
            Car {
                id,
                path,
                started_at: self.time,
                on: On::Road(start),
                waiting_for: None,
                debug: false,
            },
        );
        self.roads[start.0].cars_queue.push(id);
        true
    }

    pub fn spawn_many_on_empty_roads(&mut self, map: &Map, num_cars: usize) {
        let mut roads: Vec<RoadID> = self.roads
            .iter()
            .filter_map(|r| {
                if map.get_r(r.id.as_road()).lane_type == LaneType::Driving && r.is_empty() {
                    Some(r.id.as_road())
                } else {
                    None
                }
            })
            .collect();
        // Don't ruin determinism for silly reasons. :)
        if !roads.is_empty() {
            self.rng.shuffle(&mut roads);
        }

        let n = num_cars.min(roads.len());
        let mut actual = 0;
        for i in 0..n {
            if self.spawn_one_on_road(map, roads[i]) {
                actual += 1;
            }
        }
        println!("Spawned {} of {}", actual, n);
    }

    pub fn step(&mut self, map: &Map, control_map: &ControlMap) {
        self.time.increment();

        // Could be concurrent, since this is deterministic. Note no RNG. Ask all cars for their
        // move, reinterpreting Goto to see if there's room now. It's important to query
        // has_room_now here using the previous, fixed state of the world. If we did it in the next
        // loop, then order of updates would matter for more than just conflict resolution.
        //
        // Note that since this uses RNG right now, it's only deterministic if iteration order is!
        // So can't be concurrent and use RNG. Could have a RNG per car or something later if we
        // really needed both.
        let mut requested_moves: Vec<(CarID, Action)> = Vec::new();
        for c in self.cars.values() {
            requested_moves.push((
                c.id,
                match c.step(map, self.time) {
                    Action::Goto(on) => {
                        // This is a monotonic property in conjunction with
                        // new_car_entered_this_step. The last car won't go backwards.
                        let has_room_now = match on {
                            On::Road(id) => self.roads[id.0].room_at_end(self.time, &self.cars),
                            On::Turn(id) => self.turns[id.0].room_at_end(self.time, &self.cars),
                        };
                        let is_lead_vehicle = match c.on {
                            On::Road(id) => self.roads[id.0].cars_queue[0] == c.id,
                            On::Turn(id) => self.turns[id.0].cars_queue[0] == c.id,
                        };
                        if has_room_now && is_lead_vehicle {
                            Action::Goto(on)
                        } else {
                            Action::WaitFor(on)
                        }
                    }
                    x => x,
                },
            ));
        }
        // TODO since self.cars is a hash, requested_moves is in random order. sort by car ID to be
        // deterministic.
        requested_moves.sort_by_key(|pair| (pair.0).0);

        // Apply moves, resolving conflicts. This has to happen serially.
        // It might make more sense to push the conflict resolution down to SimQueue?
        // TODO should shuffle deterministically here, to be more fair
        let mut new_car_entered_this_step = HashSet::new();
        for (id, act) in &requested_moves {
            match *act {
                Action::Vanish => {
                    self.cars.remove(&id);
                }
                Action::Continue => {}
                Action::Goto(on) => {
                    // Order matters due to can_do_turn being mutable and due to
                    // new_car_entered_this_step.
                    let mut ok_to_turn = true;
                    if let On::Turn(t) = on {
                        ok_to_turn = self.intersections[map.get_t(t).parent.0].can_do_turn(
                            *id,
                            t,
                            self.time,
                            map,
                            control_map,
                        );
                    }

                    if new_car_entered_this_step.contains(&on) || !ok_to_turn {
                        self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
                    } else {
                        new_car_entered_this_step.insert(on);
                        let c = self.cars.get_mut(&id).unwrap();
                        if let On::Turn(t) = c.on {
                            self.intersections[map.get_t(t).parent.0].on_exit(c.id);
                            assert_eq!(c.path[0], map.get_t(t).dst);
                            c.path.pop_front();
                        }
                        c.waiting_for = None;
                        c.on = on;
                        if let On::Turn(t) = c.on {
                            self.intersections[map.get_t(t).parent.0].on_enter(c.id);
                        }
                        // TODO could calculate leftover (and deal with large timesteps, small
                        // roads)
                        c.started_at = self.time;
                    }
                }
                Action::WaitFor(on) => {
                    self.cars.get_mut(&id).unwrap().waiting_for = Some(on);
                }
            }
        }

        // Group cars by road and turn
        // TODO ideally, just hash On
        let mut cars_per_road = MultiMap::new();
        let mut cars_per_turn = MultiMap::new();
        for c in self.cars.values() {
            match c.on {
                On::Road(id) => cars_per_road.insert(id, c.id),
                On::Turn(id) => cars_per_turn.insert(id, c.id),
            };
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

    pub fn is_moving(&self, c: CarID) -> bool {
        self.cars[&c].waiting_for.is_none()
    }

    pub fn get_draw_cars_on_road(&self, r: RoadID, map: &Map) -> Vec<DrawCar> {
        self.roads[r.0].get_draw_cars(&self, map)
    }

    pub fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCar> {
        self.turns[t.0].get_draw_cars(&self, map)
    }

    pub fn summary(&self) -> String {
        let waiting = self.cars
            .values()
            .filter(|c| c.waiting_for.is_some())
            .count();
        format!(
            "Time: {0:.2}, {1} / {2} cars waiting",
            self.time,
            waiting,
            self.cars.len()
        )
    }

    pub fn car_tooltip(&self, car: CarID) -> Vec<String> {
        self.cars[&car].tooltip_lines()
    }

    pub fn toggle_debug(&mut self, car: CarID) {
        if let Some(c) = self.debug {
            if c != car {
                self.cars.get_mut(&c).unwrap().debug = false;
            }
        }

        let c = self.cars.get_mut(&car).unwrap();
        c.debug = !c.debug;
        self.debug = Some(car);
    }

    pub fn start_benchmark(&self) -> Benchmark {
        Benchmark {
            last_real_time: Instant::now(),
            last_sim_time: self.time,
        }
    }

    pub fn measure_speed(&self, b: &mut Benchmark) -> f64 {
        let elapsed = b.last_real_time.elapsed();
        let dt = (elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) * 1e-9) * si::S;
        let speed = (self.time - b.last_sim_time).as_time() / dt;
        b.last_real_time = Instant::now();
        b.last_sim_time = self.time;
        speed.value_unsafe
    }
}

pub struct Benchmark {
    last_real_time: Instant,
    last_sim_time: Tick,
}

impl Benchmark {
    pub fn has_real_time_passed(&self, d: Duration) -> bool {
        self.last_real_time.elapsed() >= d
    }
}
