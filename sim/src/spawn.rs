use map_model;
use map_model::{LaneID, Map};
use parking::ParkingSimState;
use rand::Rng;
use sim::{CarParking, DrivingModel};
use std::collections::VecDeque;
use std::time::Instant;
use walking::WalkingSimState;
use {AgentID, CarID, PedestrianID, Tick};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct Command {
    at: Tick,
    agent: AgentID,
    start: LaneID,
    goal: LaneID,
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
        driving_sim: &mut DrivingModel,
    ) {
        for p in self.spawn_parked_cars.drain(0..) {
            parking_sim.add_parked_car(p);
        }

        let mut spawn_agents: Vec<AgentID> = Vec::new();
        let mut requested_paths: Vec<(LaneID, LaneID)> = Vec::new();
        loop {
            let pop = if let Some(cmd) = self.commands.front() {
                if now == cmd.at {
                    spawn_agents.push(cmd.agent);
                    requested_paths.push((cmd.start, cmd.goal));
                    true
                } else {
                    false
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
        if spawn_agents.is_empty() {
            return;
        }
        let paths = calculate_paths(&requested_paths, map);

        let mut spawned_agents = 0;
        for (agent, (req, maybe_path)) in spawn_agents.iter().zip(requested_paths.iter().zip(paths))
        {
            if let Some(path) = maybe_path {
                match agent {
                    AgentID::Car(car) => {
                        let driving_lane = path[0];
                        let parking_lane = map.get_parent(driving_lane)
                            .find_parking_lane(driving_lane)
                            .unwrap();
                        let spot = parking_sim.get_spot_of_car(*car, parking_lane);

                        if driving_sim.start_car_on_lane(
                            now,
                            *car,
                            CarParking::new(*car, spot),
                            VecDeque::from(path),
                            map,
                        ) {
                            parking_sim.remove_parked_car(parking_lane, *car);
                            spawned_agents += 1;
                        } else {
                            // Try again next tick. Because we already slurped up all the commands
                            // for this tick, the front of the queue is the right spot.
                            self.commands.push_front(Command {
                                at: now.next(),
                                agent: *agent,
                                start: req.0,
                                goal: req.1,
                            });
                        }
                    }
                    AgentID::Pedestrian(ped) => {
                        walking_sim.seed_pedestrian(*ped, map, VecDeque::from(path));
                        spawned_agents += 1;
                    }
                };
            } else {
                println!(
                    "Couldn't find path from {} to {} for {:?}",
                    req.0, req.1, agent
                );
            }
        }
        println!(
            "Spawned {} agents of requested {}",
            spawned_agents,
            spawn_agents.len()
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

    pub fn start_parked_car<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        car: CarID,
        parking_sim: &ParkingSimState,
        rng: &mut R,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.at);
        }

        let parking_lane = parking_sim.lane_of_car(car).expect("Car isn't parked");
        let road = map.get_parent(parking_lane);
        let driving_lane = road.find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        let goal = pick_goal(rng, map, driving_lane);
        // TODO avoid dupe commands
        self.commands.push_back(Command {
            at,
            agent: AgentID::Car(car),
            start: driving_lane,
            goal,
        });
    }

    pub fn start_many_parked_cars<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        num: usize,
        rng: &mut R,
        parking_sim: &ParkingSimState,
    ) {
        let mut cars: Vec<CarID> = parking_sim
            .get_all_cars()
            .into_iter()
            .filter_map(|(car, parking_lane)| {
                map.get_parent(parking_lane)
                    .find_driving_lane(parking_lane)
                    .and_then(|_driving_lane| Some(car))
            })
            .collect();
        if cars.is_empty() {
            return;
        }
        rng.shuffle(&mut cars);

        for car in &cars[0..num.min(cars.len())] {
            self.start_parked_car(at, map, *car, parking_sim, rng);
        }
    }

    pub fn spawn_pedestrian<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        sidewalk: LaneID,
        rng: &mut R,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.at);
        }
        assert!(map.get_l(sidewalk).is_sidewalk());

        let goal = pick_goal(rng, map, sidewalk);
        self.commands.push_back(Command {
            at,
            agent: AgentID::Pedestrian(PedestrianID(self.ped_id_counter)),
            start: sidewalk,
            goal,
        });
        self.ped_id_counter += 1;
    }

    pub fn spawn_many_pedestrians<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        num: usize,
        rng: &mut R,
    ) {
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

    let elapsed = timer.elapsed();
    let dt = elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) * 1e-9;
    println!("Calculating {} paths took {}s", paths.len(), dt,);
    paths
}
