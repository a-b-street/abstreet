// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use control::ControlMap;
use dimensioned::si;
use draw_car::DrawCar;
use driving::{Car, DrivingSimState, On};
use map_model;
use map_model::{LaneType, Map, RoadID, TurnID};
use rand::{FromEntropy, Rng, SeedableRng, XorShiftRng};
use std::collections::VecDeque;
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
    pub time: Tick,
    id_counter: usize,
    debug: Option<CarID>,

    driving_state: DrivingSimState,
    // TODO parking state
}

impl Sim {
    pub fn new(map: &Map, rng_seed: Option<u8>) -> Sim {
        let mut rng = XorShiftRng::from_entropy();
        if let Some(seed) = rng_seed {
            rng = XorShiftRng::from_seed([seed; 16]);
        }

        let driving_state = DrivingSimState::new(map);

        Sim {
            rng,
            driving_state,
            time: Tick::zero(),
            id_counter: 0,
            debug: None,
        }
    }

    // TODO cars basically start in the intersection, with their front bumper right at the
    // beginning of the road. later, we want cars starting at arbitrary points in the middle of the
    // road (from a building), so just ignore this problem for now.
    pub fn spawn_one_on_road(&mut self, map: &Map, start: RoadID) -> bool {
        if !self.driving_state.roads[start.0].room_at_end(self.time, &self.driving_state.cars) {
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

        self.driving_state.cars.insert(
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
        self.driving_state.roads[start.0].cars_queue.push(id);
        true
    }

    pub fn spawn_many_on_empty_roads(&mut self, map: &Map, num_cars: usize) {
        let mut roads: Vec<RoadID> = self.driving_state
            .roads
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

        self.driving_state.step(self.time, map, control_map);
    }

    pub fn is_moving(&self, c: CarID) -> bool {
        // TODO dont assume driving state
        self.driving_state.cars[&c].waiting_for.is_none()
    }

    pub fn get_draw_cars_on_road(&self, r: RoadID, map: &Map) -> Vec<DrawCar> {
        // TODO dont assume driving state
        self.driving_state.roads[r.0].get_draw_cars(self.time, &self.driving_state, map)
    }

    pub fn get_draw_cars_on_turn(&self, t: TurnID, map: &Map) -> Vec<DrawCar> {
        // TODO dont assume driving state
        self.driving_state.turns[t.0].get_draw_cars(self.time, &self.driving_state, map)
    }

    pub fn summary(&self) -> String {
        // TODO dont assume driving state
        let waiting = self.driving_state
            .cars
            .values()
            .filter(|c| c.waiting_for.is_some())
            .count();
        format!(
            "Time: {0:.2}, {1} / {2} cars waiting",
            self.time,
            waiting,
            self.driving_state.cars.len()
        )
    }

    pub fn car_tooltip(&self, car: CarID) -> Vec<String> {
        // TODO dont assume driving state
        self.driving_state.cars[&car].tooltip_lines()
    }

    pub fn toggle_debug(&mut self, car: CarID) {
        if let Some(c) = self.debug {
            if c != car {
                self.driving_state.cars.get_mut(&c).unwrap().debug = false;
            }
        }

        let c = self.driving_state.cars.get_mut(&car).unwrap();
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
