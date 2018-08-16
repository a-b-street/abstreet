use map_model;
use map_model::{LaneID, Map};
use parking::ParkingSimState;
use rand::Rng;
use sim::CarParking;
use std::collections::VecDeque;
use std::time::Instant;
use walking::WalkingSimState;
use {CarID, PedestrianID, Tick};

// TODO move the stuff in sim that does RNG stuff, picks goals, etc to here. make the UI commands
// funnel into here and do stuff on the next tick.

#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum Command {
    // goal lane
    StartParkedCar(Tick, CarID, LaneID),
    // start, goal lanes
    SpawnPedestrian(Tick, PedestrianID, LaneID, LaneID),
}

impl Command {
    fn get_time(&self) -> Tick {
        match self {
            Command::StartParkedCar(time, _, _) => *time,
            Command::SpawnPedestrian(time, _, _, _) => *time,
        }
    }
}

// This must get the car/ped IDs correct.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Spawner {
    // This happens immediately (at the beginning of the simulation in most cases, except for
    // interactive UI stuff)
    spawn_parked_cars: Vec<CarParking>,

    // Ordered by time
    commands: VecDeque<Command>,

    car_id_counter: usize,
    ped_id_counter: usize,
}

impl Spawner {
    pub fn empty() -> Spawner {
        Spawner {
            spawn_parked_cars: Vec::new(),
            commands: VecDeque::new(),
            car_id_counter: 0,
            ped_id_counter: 0,
        }
    }

    pub fn step(
        &mut self,
        now: Tick,
        map: &Map,
        parking_sim: &mut ParkingSimState,
        walking_sim: &mut WalkingSimState,
    ) {
        for p in self.spawn_parked_cars.drain(0..) {
            parking_sim.add_parked_car(p);
        }

        let mut spawn_peds: Vec<PedestrianID> = Vec::new();
        let mut requested_paths: Vec<(LaneID, LaneID)> = Vec::new();
        loop {
            let pop = if let Some(cmd) = self.commands.front() {
                match cmd {
                    Command::StartParkedCar(time, car, goal) => {
                        println!("TODO");
                        false
                    }
                    Command::SpawnPedestrian(time, ped, start, goal) => {
                        if now == *time {
                            spawn_peds.push(*ped);
                            requested_paths.push((*start, *goal));
                            true
                        } else {
                            false
                        }
                    }
                }
            } else {
                false
            };
            if pop {
                self.commands.pop_front();
            } else {
                break;
            }
        }

        if requested_paths.is_empty() {
            return;
        }

        let paths = calculate_paths(&requested_paths, map);
        let mut spawned_peds = 0;
        for (ped, (req, maybe_path)) in spawn_peds.iter().zip(requested_paths.iter().zip(paths)) {
            if let Some(path) = maybe_path {
                walking_sim.seed_pedestrian(*ped, map, VecDeque::from(path));
                spawned_peds += 1;
            } else {
                println!("Couldn't find path from {} to {} for {}", req.0, req.1, ped);
            }
        }
        println!(
            "Spawned {} pedestrians of requested {}",
            spawned_peds,
            spawn_peds.len()
        );
    }

    // TODO the mut is temporary
    pub fn seed_parked_cars<R: Rng + ?Sized>(
        &mut self,
        percent_capacity_to_fill: f64,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
    ) {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);
        assert!(self.spawn_parked_cars.is_empty());

        let mut total_capacity = 0;
        let mut new_cars = 0;
        for spot in parking_sim.get_all_free_spots() {
            total_capacity += 1;
            if rng.gen_bool(percent_capacity_to_fill) {
                new_cars += 1;
                // TODO since spawning applies during the next step, lots of stuff breaks without
                // this :(
                parking_sim.add_parked_car(CarParking::new(CarID(self.car_id_counter), spot));
                //self.spawn_parked_cars.push(CarParking::new(CarID(self.car_id_counter), spot));
                self.car_id_counter += 1;
            }
        }

        println!(
            "Seeded {} of {} parking spots with cars",
            new_cars, total_capacity
        );
    }

    pub fn spawn_pedestrian<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        sidewalk: LaneID,
        rng: &mut R,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.get_time());
        }
        assert!(map.get_l(sidewalk).is_sidewalk());

        let goal = pick_goal(rng, map, sidewalk);
        self.commands.push_back(Command::SpawnPedestrian(
            at,
            PedestrianID(self.ped_id_counter),
            sidewalk,
            goal,
        ));
        self.ped_id_counter += 1;
        println!("Spawned a pedestrian at {}", sidewalk);
    }

    pub fn spawn_many_pedestrians<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        num: usize,
        rng: &mut R,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.get_time());
        }

        let mut sidewalks: Vec<LaneID> = Vec::new();
        for l in map.all_lanes() {
            if l.is_sidewalk() {
                sidewalks.push(l.id);
            }
        }

        for _i in 0..num {
            let start = *rng.choose(&sidewalks).unwrap();
            self.spawn_pedestrian(at, map, start, rng);
        }
    }
}

fn pick_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> LaneID {
    let lane_type = map.get_l(start).lane_type;
    let candidate_goals: Vec<LaneID> = map.all_lanes()
        .iter()
        .filter_map(|l| {
            if l.lane_type != lane_type || l.id == start {
                None
            } else {
                Some(l.id)
            }
        })
        .collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn calculate_paths(requested_paths: &Vec<(LaneID, LaneID)>, map: &Map) -> Vec<Option<Vec<LaneID>>> {
    use rayon::prelude::*;

    println!("Calculating {} paths", requested_paths.len());
    // TODO better timer macro
    let timer = Instant::now();
    let paths: Vec<Option<Vec<LaneID>>> = requested_paths
        .par_iter()
        .map(|(start, goal)| map_model::pathfind(map, *start, *goal))
        .collect();

    println!(
        "Calculating {} paths took {:?}",
        paths.len(),
        timer.elapsed()
    );
    paths
}
