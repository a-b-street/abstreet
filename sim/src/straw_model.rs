// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use common::{CarID, Tick, SPEED_LIMIT};
use control::ControlMap;
use dimensioned::si;
use ezgui::GfxCtx;
use geom::{geometry, GeomMap, Radian};
use graphics;
use graphics::math::Vec2d;
use map_model::{LaneType, Map, Pt2D, RoadID, TurnID};
use multimap::MultiMap;
use rand::{FromEntropy, Rng, SeedableRng, XorShiftRng};
use std::collections::{BTreeMap, HashSet};
use std::f64;
use std::time::{Duration, Instant};
use straw_intersections::{IntersectionPolicy, StopSign, TrafficSignal};

use std;
const FOLLOWING_DISTANCE: si::Meter<f64> = si::Meter {
    value_unsafe: 8.0,
    _marker: std::marker::PhantomData,
};

const CAR_WIDTH: f64 = 2.0;
const CAR_LENGTH: f64 = 4.5;

// TODO move this out
pub struct DrawCar {
    pub id: CarID,
    quad: Vec<Vec2d>,
    front: Pt2D,
    // TODO ideally, draw the turn icon inside the car quad. how can we do that easily?
    turn_arrow: Option<[f64; 4]>,
}

impl DrawCar {
    fn new(id: CarID, front: &Pt2D, angle: Radian<f64>) -> DrawCar {
        DrawCar {
            id,
            front: front.clone(),
            // Fill this out later
            turn_arrow: None,
            // TODO the rounded corners from graphics::Line::new_round look kind of cool though
            // add PI because we want to find the back of the car relative to the front
            quad: geometry::thick_line_from_angle(
                CAR_WIDTH,
                CAR_LENGTH,
                front,
                angle + (f64::consts::PI * geometry::angles::RAD),
            ),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        let poly = graphics::Polygon::new(color);
        poly.draw(&self.quad, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        // TODO tune color, sizes
        if let Some(a) = self.turn_arrow {
            let turn_line = graphics::Line::new_round([0.0, 1.0, 1.0, 1.0], 0.25);
            turn_line.draw_arrow(a, 1.0, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.quad)
    }
}

// TODO this name isn't quite right :)
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
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

    fn length(&self, geom_map: &GeomMap) -> si::Meter<f64> {
        match self {
            &On::Road(id) => geom_map.get_r(id).length(),
            &On::Turn(id) => geom_map.get_t(id).length(),
        }
    }

    fn dist_along(&self, dist: si::Meter<f64>, geom_map: &GeomMap) -> (Pt2D, Radian<f64>) {
        match self {
            &On::Road(id) => geom_map.get_r(id).dist_along(dist),
            &On::Turn(id) => geom_map.get_t(id).dist_along(dist),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Car {
    id: CarID,
    on: On,
    // When did the car start the current On?
    started_at: Tick,
    // TODO ideally, something else would remember Goto was requested and not even call step()
    waiting_for: Option<On>,
    debug: bool,
}

enum Action {
    Vanish,      // hit a deadend, oops
    Continue,    // need more time to cross the current spot
    Goto(On),    // go somewhere if there's room
    WaitFor(On), // TODO this is only used inside sim. bleh.
}

impl Car {
    fn tooltip_lines(&self) -> Vec<String> {
        vec![
            format!("Car {:?}", self.id),
            format!("On {:?}, started at {:?}", self.on, self.started_at),
            format!("Committed to waiting for {:?}", self.waiting_for),
        ]
    }

    fn step(&self, geom_map: &GeomMap, map: &Map, time: Tick, rng: &mut XorShiftRng) -> Action {
        if let Some(on) = self.waiting_for {
            return Action::Goto(on);
        }

        let dist = SPEED_LIMIT * (time - self.started_at).as_time();
        if dist < self.on.length(geom_map) {
            return Action::Continue;
        }

        match self.on {
            // For now, just kill off cars that're stuck on disconnected bits of the map
            On::Road(id) if map.get_turns_from_road(id).is_empty() => Action::Vanish,
            // TODO cant try to go to next road unless we're the front car
            // if we dont do this here, we wont be able to see what turns people are waiting for
            // even if we wait till we're the front car, we might unravel the line of queued cars
            // too quickly
            On::Road(id) => Action::Goto(On::Turn(self.choose_turn(id, map, rng))),
            On::Turn(id) => Action::Goto(On::Road(map.get_t(id).dst)),
        }
    }

    fn choose_turn(&self, from: RoadID, map: &Map, rng: &mut XorShiftRng) -> TurnID {
        assert!(self.waiting_for.is_none());
        rng.choose(&map.get_turns_from_road(from)).unwrap().id
    }

    // Returns the angle and the dist along the road/turn too
    fn get_best_case_pos(
        &self,
        time: Tick,
        geom_map: &GeomMap,
    ) -> (Pt2D, Radian<f64>, si::Meter<f64>) {
        let mut dist = SPEED_LIMIT * (time - self.started_at).as_time();
        if self.waiting_for.is_some() {
            dist = self.on.length(geom_map);
        }
        let (pt, angle) = self.on.dist_along(dist, geom_map);
        (pt, angle, dist)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct SimQueue {
    id: On,
    cars_queue: Vec<CarID>,
    capacity: usize,
}

impl SimQueue {
    fn new(id: On, geom_map: &GeomMap) -> SimQueue {
        SimQueue {
            id,
            cars_queue: Vec::new(),
            capacity: ((id.length(geom_map) / FOLLOWING_DISTANCE).floor() as usize).max(1),
        }
    }

    // TODO it'd be cool to contribute tooltips (like number of cars currently here, capacity) to
    // tooltip

    fn room_at_end(&self, time: Tick, cars: &BTreeMap<CarID, Car>) -> bool {
        if self.cars_queue.is_empty() {
            return true;
        }
        if self.cars_queue.len() == self.capacity {
            return false;
        }
        // Has the last car crossed at least FOLLOWING_DISTANCE? If so and the capacity
        // isn't filled, then we know for sure that there's room, because in this model, we assume
        // none of the cars just arbitrarily slow down or stop without reason.
        (time - cars[self.cars_queue.last().unwrap()].started_at).as_time()
            >= FOLLOWING_DISTANCE / SPEED_LIMIT
    }

    fn reset(&mut self, ids: &Vec<CarID>, cars: &BTreeMap<CarID, Car>) {
        let old_queue = self.cars_queue.clone();

        assert!(ids.len() <= self.capacity);
        self.cars_queue.clear();
        self.cars_queue.extend(ids);
        self.cars_queue.sort_by_key(|id| cars[id].started_at);

        // assert here we're not squished together too much
        let min_dt = FOLLOWING_DISTANCE / SPEED_LIMIT;
        for slice in self.cars_queue.windows(2) {
            let c1 = cars[&slice[0]].started_at.as_time();
            let c2 = cars[&slice[1]].started_at.as_time();
            if c2 - c1 < min_dt {
                println!("uh oh! on {:?}, reset to {:?} broke. min dt is {}, but we have {} and {}. badness {}", self.id, self.cars_queue, min_dt, c2, c1, c2 - c1 - min_dt);
                println!("  prev queue was {:?}", old_queue);
                for c in &self.cars_queue {
                    println!("  {:?} started at {}", c, cars[c].started_at);
                }
                panic!("invariant borked");
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.cars_queue.is_empty()
    }

    // TODO this starts cars with their front aligned with the end of the road, sticking their back
    // into the intersection. :(
    fn get_draw_cars(&self, sim: &Sim, geom_map: &GeomMap) -> Vec<DrawCar> {
        if self.cars_queue.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let (pos1, angle1, dist_along1) =
            sim.cars[&self.cars_queue[0]].get_best_case_pos(sim.time, geom_map);
        results.push(DrawCar::new(self.cars_queue[0], &pos1, angle1));
        let mut dist_along_bound = dist_along1;

        for id in self.cars_queue.iter().skip(1) {
            let (pos, angle, dist_along) = sim.cars[id].get_best_case_pos(sim.time, geom_map);
            if dist_along_bound - FOLLOWING_DISTANCE > dist_along {
                results.push(DrawCar::new(*id, &pos, angle));
                dist_along_bound = dist_along;
            } else {
                dist_along_bound -= FOLLOWING_DISTANCE;
                // If not, we violated room_at_end() and reset() didn't catch it
                assert!(dist_along_bound >= 0.0 * si::M, "dist_along_bound went negative ({}) for {:?} (length {}) with queue {:?}. first car at {}", dist_along_bound, self.id, self.id.length(geom_map), self.cars_queue, dist_along1);
                let (pt, angle) = self.id.dist_along(dist_along_bound, geom_map);
                results.push(DrawCar::new(*id, &pt, angle));
            }
        }

        results
    }
}

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
    cars: BTreeMap<CarID, Car>,
    roads: Vec<SimQueue>,
    turns: Vec<SimQueue>,
    intersections: Vec<IntersectionPolicy>,
    pub time: Tick,
    id_counter: usize,
    debug: Option<CarID>,
}

impl Sim {
    pub fn new(map: &Map, geom_map: &GeomMap, rng_seed: Option<u8>) -> Sim {
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
            roads: map.all_roads()
                .iter()
                .map(|r| SimQueue::new(On::Road(r.id), geom_map))
                .collect(),
            turns: map.all_turns()
                .iter()
                .map(|t| SimQueue::new(On::Turn(t.id), geom_map))
                .collect(),
            time: Tick::zero(),
            id_counter: 0,
            debug: None,
        }
    }

    // TODO cars basically start in the intersection, with their front bumper right at the
    // beginning of the road. later, we want cars starting at arbitrary points in the middle of the
    // road (from a building), so just ignore this problem for now.
    pub fn spawn_one_on_road(&mut self, road: RoadID) -> bool {
        if !self.roads[road.0].room_at_end(self.time, &self.cars) {
            return false;
        }
        let id = CarID(self.id_counter);
        self.id_counter += 1;
        self.cars.insert(
            id,
            Car {
                id,
                started_at: self.time,
                on: On::Road(road),
                waiting_for: None,
                debug: false,
            },
        );
        self.roads[road.0].cars_queue.push(id);
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
        for i in 0..n {
            assert!(self.spawn_one_on_road(roads[i]));
        }
        println!("Spawned {}", n);
    }

    pub fn step(&mut self, geom_map: &GeomMap, map: &Map, control_map: &ControlMap) {
        self.time.increment();

        // Could be concurrent. Ask all cars for their move, reinterpreting Goto to see if there's
        // room now. It's important to query has_room_now here using the previous, fixed state of
        // the world. If we did it in the next loop, then order of updates would matter for more
        // than just conflict resolution.
        //
        // Note that since this uses RNG right now, it's only deterministic if iteration order is!
        // So can't be concurrent and use RNG. Could have a RNG per car or something later if we
        // really needed both.
        let mut requested_moves: Vec<(CarID, Action)> = Vec::new();
        for c in self.cars.values() {
            requested_moves.push((
                c.id,
                match c.step(geom_map, map, self.time, &mut self.rng) {
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
                            geom_map,
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

    pub fn get_draw_cars_on_road(&self, r: RoadID, geom_map: &GeomMap) -> Vec<DrawCar> {
        let mut cars = self.roads[r.0].get_draw_cars(&self, geom_map);
        for c in &mut cars {
            if let Some(on) = self.cars[&c.id].waiting_for {
                let slope = geom_map.get_t(on.as_turn()).slope();
                c.turn_arrow = Some([
                    c.front.x() - (CAR_LENGTH / 2.0) * slope[0],
                    c.front.y() - (CAR_LENGTH / 2.0) * slope[1],
                    c.front.x(),
                    c.front.y(),
                ]);
            }
        }
        cars
    }

    pub fn get_draw_cars_on_turn(&self, t: TurnID, geom_map: &GeomMap) -> Vec<DrawCar> {
        self.turns[t.0].get_draw_cars(&self, geom_map)
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
