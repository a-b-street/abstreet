use driving::DrivingSimState;
use kinematics::Vehicle;
use map_model;
use map_model::{BuildingID, LaneID, Map};
use parking::ParkingSimState;
use rand::Rng;
use sim::CarParking;
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;
use walking::WalkingSimState;
use {AgentID, CarID, PedestrianID, Tick};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct Command {
    at: Tick,
    agent: AgentID,
    start: BuildingID,
    goal: BuildingID,
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
        driving_sim: &mut DrivingSimState,
        properties: &BTreeMap<CarID, Vehicle>,
    ) {
        for p in self.spawn_parked_cars.drain(0..) {
            parking_sim.add_parked_car(p);
        }

        let mut spawn_agents: Vec<(AgentID, BuildingID, BuildingID)> = Vec::new();
        let mut requested_paths: Vec<(LaneID, LaneID)> = Vec::new();
        loop {
            let pop = if let Some(cmd) = self.commands.front() {
                if now == cmd.at {
                    spawn_agents.push((cmd.agent, cmd.start, cmd.goal));
                    let (start_lane, goal_lane) = match cmd.agent {
                        AgentID::Car(_) => (
                            map.get_driving_lane_from_bldg(cmd.start).unwrap(),
                            map.get_driving_lane_from_bldg(cmd.goal).unwrap(),
                        ),
                        AgentID::Pedestrian(_) => (
                            map.get_b(cmd.start).front_path.sidewalk,
                            map.get_b(cmd.goal).front_path.sidewalk,
                        ),
                    };
                    requested_paths.push((start_lane, goal_lane));
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
        for ((agent, start_bldg, goal_bldg), (req, maybe_path)) in spawn_agents
            .into_iter()
            .zip(requested_paths.iter().zip(paths))
        {
            if let Some(path) = maybe_path {
                match agent {
                    AgentID::Car(car) => {
                        let driving_lane = path[0];
                        let parking_lane = map.get_parent(driving_lane)
                            .find_parking_lane(driving_lane)
                            .unwrap();
                        let spot = parking_sim.get_spot_of_car(car, parking_lane);

                        if driving_sim.start_car_on_lane(
                            now,
                            car,
                            CarParking::new(car, spot),
                            VecDeque::from(path),
                            map,
                            properties,
                        ) {
                            parking_sim.remove_parked_car(parking_lane, car);
                            spawned_agents += 1;
                        } else {
                            // Try again next tick. Because we already slurped up all the commands
                            // for this tick, the front of the queue is the right spot.
                            self.commands.push_front(Command {
                                at: now.next(),
                                agent: agent,
                                start: start_bldg,
                                goal: goal_bldg,
                            });
                        }
                    }
                    AgentID::Pedestrian(ped) => {
                        walking_sim.seed_pedestrian(ped, start_bldg, map, VecDeque::from(path));
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
            requested_paths.len()
        );
    }

    // TODO the mut is temporary
    pub fn seed_parked_cars<R: Rng + ?Sized>(
        &mut self,
        percent_capacity_to_fill: f64,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
    ) -> Vec<Vehicle> {
        assert!(percent_capacity_to_fill >= 0.0 && percent_capacity_to_fill <= 1.0);
        assert!(self.spawn_parked_cars.is_empty());

        let mut total_capacity = 0;
        let mut new_cars: Vec<Vehicle> = Vec::new();
        for spot in parking_sim.get_all_free_spots() {
            total_capacity += 1;
            if rng.gen_bool(percent_capacity_to_fill) {
                let id = CarID(self.car_id_counter);
                // TODO since spawning applies during the next step, lots of stuff breaks without
                // this :(
                parking_sim.add_parked_car(CarParking::new(id, spot));
                //self.spawn_parked_cars.push(CarParking::new(CarID(self.car_id_counter), spot));
                new_cars.push(Vehicle::generate_typical_car(id, rng));
                self.car_id_counter += 1;
            }
        }

        println!(
            "Seeded {} of {} parking spots with cars",
            new_cars.len(),
            total_capacity
        );
        new_cars
    }

    pub fn seed_specific_parked_cars<R: Rng + ?Sized>(
        &mut self,
        lane: LaneID,
        spot_indices: Vec<usize>,
        parking_sim: &mut ParkingSimState,
        rng: &mut R,
    ) -> Vec<Vehicle> {
        assert!(self.spawn_parked_cars.is_empty());
        let spots = parking_sim.get_all_spots(lane);
        spot_indices
            .into_iter()
            .map(|idx| {
                let id = CarID(self.car_id_counter);
                parking_sim.add_parked_car(CarParking::new(id, spots[idx].clone()));
                // TODO push onto spawn_parked_cars?
                self.car_id_counter += 1;
                Vehicle::generate_typical_car(id, rng)
            })
            .collect()
    }

    pub fn start_parked_car_with_goal<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        car: CarID,
        parking_sim: &ParkingSimState,
        goal: LaneID,
        rng: &mut R,
    ) {
        if let Some(cmd) = self.commands.back() {
            assert!(at >= cmd.at);
        }
        // Don't add duplicate commands.
        if self.commands
            .iter()
            .find(|cmd| cmd.agent == AgentID::Car(car))
            .is_some()
        {
            println!(
                "{} is already scheduled to start, ignoring new request",
                car
            );
            return;
        }

        let parking_lane = parking_sim.lane_of_car(car).expect("Car isn't parked");
        let road = map.get_parent(parking_lane);
        let driving_lane = road.find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        self.commands.push_back(Command {
            at,
            agent: AgentID::Car(car),
            start: pick_bldg_from_driving_lane(rng, map, driving_lane),
            goal: pick_bldg_from_driving_lane(rng, map, goal),
        });
    }

    pub fn start_parked_car<R: Rng + ?Sized>(
        &mut self,
        at: Tick,
        map: &Map,
        car: CarID,
        parking_sim: &ParkingSimState,
        rng: &mut R,
    ) {
        let parking_lane = parking_sim.lane_of_car(car).expect("Car isn't parked");
        let road = map.get_parent(parking_lane);
        let driving_lane = road.find_driving_lane(parking_lane)
            .expect("Parking lane has no driving lane");

        let goal = pick_car_goal(rng, map, driving_lane);
        self.start_parked_car_with_goal(at, map, car, parking_sim, goal, rng);
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
                let has_bldgs = map.get_parent(parking_lane)
                    .find_sidewalk(parking_lane)
                    .and_then(|sidewalk| Some(!map.get_l(sidewalk).building_paths.is_empty()))
                    .unwrap_or(false);
                if has_bldgs {
                    map.get_parent(parking_lane)
                        .find_driving_lane(parking_lane)
                        .and_then(|_driving_lane| Some(car))
                } else {
                    None
                }
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

        self.commands.push_back(Command {
            at,
            agent: AgentID::Pedestrian(PedestrianID(self.ped_id_counter)),
            start: pick_bldg_from_sidewalk(rng, map, sidewalk),
            goal: pick_ped_goal(rng, map, sidewalk),
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
            if l.is_sidewalk() && !l.building_paths.is_empty() {
                sidewalks.push(l.id);
            }
        }

        for _i in 0..num {
            let start = *rng.choose(&sidewalks).unwrap();
            self.spawn_pedestrian(at, map, start, rng);
        }
    }
}

fn pick_car_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> LaneID {
    let candidate_goals: Vec<LaneID> = map.all_lanes()
        .iter()
        .filter_map(|l| {
            if l.id != start && l.is_driving() {
                if let Some(sidewalk) = map.get_sidewalk_from_driving_lane(l.id) {
                    if !map.get_l(sidewalk).building_paths.is_empty() {
                        return Some(l.id);
                    }
                }
            }
            None
        })
        .collect();
    *rng.choose(&candidate_goals).unwrap()
}

fn pick_ped_goal<R: Rng + ?Sized>(rng: &mut R, map: &Map, start: LaneID) -> BuildingID {
    let candidate_goals: Vec<BuildingID> = map.all_buildings()
        .iter()
        .filter_map(|b| {
            if b.front_path.sidewalk != start {
                Some(b.id)
            } else {
                None
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

fn pick_bldg_from_sidewalk<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    sidewalk: LaneID,
) -> BuildingID {
    *rng.choose(&map.get_l(sidewalk).building_paths)
        .expect(&format!("{} has no buildings", sidewalk))
}

fn pick_bldg_from_driving_lane<R: Rng + ?Sized>(
    rng: &mut R,
    map: &Map,
    start: LaneID,
) -> BuildingID {
    pick_bldg_from_sidewalk(rng, map, map.get_sidewalk_from_driving_lane(start).unwrap())
}
