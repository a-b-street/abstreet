use crate::{
    CarID, DrivingGoal, ParkingSpot, SidewalkSpot, Sim, TripSpec, VehicleSpec, VehicleType,
    BIKE_LENGTH, MAX_CAR_LENGTH, MIN_CAR_LENGTH,
};
use abstutil;
use abstutil::{fork_rng, prettyprint_usize, Timer, WeightedUsizeChoice};
use geom::{Distance, Duration, Speed};
use map_model::{
    BuildingID, BusRouteID, BusStopID, FullNeighborhoodInfo, IntersectionID, LaneType, Map,
    Position, RoadID,
};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: String,

    pub seed_parked_cars: Vec<SeedParkedCars>,
    pub spawn_over_time: Vec<SpawnOverTime>,
    pub border_spawn_over_time: Vec<BorderSpawnOverTime>,
    pub individ_trips: Vec<SpawnTrip>,
}

// SpawnOverTime and BorderSpawnOverTime should be kept separate. Agents in SpawnOverTime pick
// their mode (use a car, walk, bus) based on the situation. When spawning directly a border,
// agents have to start as a car or pedestrian already.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SpawnOverTime {
    pub num_agents: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_time: Duration,
    pub stop_time: Duration,
    pub start_from_neighborhood: String,
    pub goal: OriginDestination,
    pub percent_biking: f64,
    pub percent_use_transit: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BorderSpawnOverTime {
    pub num_peds: usize,
    pub num_cars: usize,
    pub num_bikes: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_time: Duration,
    pub stop_time: Duration,
    // TODO A serialized Scenario won't last well as the map changes...
    pub start_from_border: IntersectionID,
    pub goal: OriginDestination,
    pub percent_use_transit: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SeedParkedCars {
    pub neighborhood: String,
    pub cars_per_building: WeightedUsizeChoice,
}

impl Scenario {
    pub fn describe(&self) -> Vec<String> {
        vec![
            format!("{} for {}", self.scenario_name, self.map_name),
            format!(
                "{} SeedParkedCars",
                prettyprint_usize(self.seed_parked_cars.len())
            ),
            format!(
                "{} SpawnOverTime",
                prettyprint_usize(self.spawn_over_time.len())
            ),
            format!(
                "{} BorderSpawnOverTime",
                prettyprint_usize(self.border_spawn_over_time.len())
            ),
            format!("{} SpawnTrip", prettyprint_usize(self.individ_trips.len())),
        ]
    }

    // TODO may need to fork the RNG a bit more
    pub fn instantiate(&self, sim: &mut Sim, map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) {
        sim.set_name(self.scenario_name.clone());

        timer.start(&format!("Instantiating {}", self.scenario_name));

        for route in map.get_all_bus_routes() {
            sim.seed_bus_route(route, map, timer);
        }

        timer.start("load full neighborhood info");
        let neighborhoods = FullNeighborhoodInfo::load_all(map);
        timer.stop("load full neighborhood info");

        for s in &self.seed_parked_cars {
            if !neighborhoods.contains_key(&s.neighborhood) {
                panic!("Neighborhood {} isn't defined", s.neighborhood);
            }

            seed_parked_cars(
                sim,
                &s.cars_per_building,
                &neighborhoods[&s.neighborhood].buildings,
                &neighborhoods[&s.neighborhood].roads,
                rng,
                map,
                timer,
            );
        }

        // Don't let two pedestrians starting from one building use the same car.
        let mut reserved_cars: HashSet<CarID> = HashSet::new();

        for s in &self.spawn_over_time {
            if !neighborhoods.contains_key(&s.start_from_neighborhood) {
                panic!("Neighborhood {} isn't defined", s.start_from_neighborhood);
            }

            timer.start_iter("SpawnOverTime each agent", s.num_agents);
            for _ in 0..s.num_agents {
                timer.next();
                s.spawn_agent(rng, sim, &mut reserved_cars, &neighborhoods, map, timer);
            }
        }

        timer.start_iter("BorderSpawnOverTime", self.border_spawn_over_time.len());
        for s in &self.border_spawn_over_time {
            timer.next();
            s.spawn_peds(rng, sim, &neighborhoods, map, timer);
            s.spawn_cars(rng, sim, &neighborhoods, map, timer);
            s.spawn_bikes(rng, sim, &neighborhoods, map, timer);
        }

        timer.start_iter("SpawnTrip", self.individ_trips.len());
        for t in &self.individ_trips {
            match t.clone() {
                SpawnTrip::CarAppearing {
                    depart,
                    start,
                    goal,
                    is_bike,
                    ..
                } => {
                    sim.schedule_trip(
                        depart,
                        TripSpec::CarAppearing {
                            start_pos: start,
                            goal,
                            vehicle_spec: if is_bike {
                                Scenario::rand_bike(rng)
                            } else {
                                Scenario::rand_car(rng)
                            },
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                    );
                }
                SpawnTrip::UsingBike(depart, start, goal) => {
                    sim.schedule_trip(
                        depart,
                        TripSpec::UsingBike {
                            start,
                            goal,
                            vehicle: Scenario::rand_bike(rng),
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                    );
                }
                SpawnTrip::JustWalking(depart, start, goal) => {
                    sim.schedule_trip(
                        depart,
                        TripSpec::JustWalking {
                            start,
                            goal,
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                    );
                }
                SpawnTrip::UsingTransit(depart, start, goal, route, stop1, stop2) => {
                    sim.schedule_trip(
                        depart,
                        TripSpec::UsingTransit {
                            start,
                            goal,
                            route,
                            stop1,
                            stop2,
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                    );
                }
            }
            timer.next();
        }

        sim.spawn_all_trips(map, timer, true);
        timer.stop(&format!("Instantiating {}", self.scenario_name));
    }

    pub fn save(&self) {
        abstutil::save_binary_object(
            abstutil::SCENARIOS,
            &self.map_name,
            &self.scenario_name,
            self,
        );
    }

    pub fn small_run(map: &Map) -> Scenario {
        let mut s = Scenario {
            scenario_name: "small_run".to_string(),
            map_name: map.get_name().to_string(),
            seed_parked_cars: vec![SeedParkedCars {
                neighborhood: "_everywhere_".to_string(),
                cars_per_building: WeightedUsizeChoice {
                    weights: vec![5, 5],
                },
            }],
            spawn_over_time: vec![SpawnOverTime {
                num_agents: 100,
                start_time: Duration::ZERO,
                stop_time: Duration::seconds(5.0),
                start_from_neighborhood: "_everywhere_".to_string(),
                goal: OriginDestination::Neighborhood("_everywhere_".to_string()),
                percent_biking: 0.5,
                percent_use_transit: 0.5,
            }],
            // If there are no sidewalks/driving lanes at a border, scenario instantiation will
            // just warn and skip them.
            border_spawn_over_time: map
                .all_incoming_borders()
                .into_iter()
                .map(|i| BorderSpawnOverTime {
                    num_peds: 10,
                    num_cars: 10,
                    num_bikes: 10,
                    start_time: Duration::ZERO,
                    stop_time: Duration::seconds(5.0),
                    start_from_border: i.id,
                    goal: OriginDestination::Neighborhood("_everywhere_".to_string()),
                    percent_use_transit: 0.5,
                })
                .collect(),
            individ_trips: Vec::new(),
        };
        for i in map.all_outgoing_borders() {
            s.spawn_over_time.push(SpawnOverTime {
                num_agents: 10,
                start_time: Duration::ZERO,
                stop_time: Duration::seconds(5.0),
                start_from_neighborhood: "_everywhere_".to_string(),
                goal: OriginDestination::Border(i.id),
                percent_biking: 0.5,
                percent_use_transit: 0.5,
            });
        }
        s
    }

    // Just buses.
    pub fn empty(map: &Map) -> Scenario {
        Scenario {
            scenario_name: "just buses".to_string(),
            map_name: map.get_name().to_string(),
            seed_parked_cars: Vec::new(),
            spawn_over_time: Vec::new(),
            border_spawn_over_time: Vec::new(),
            individ_trips: Vec::new(),
        }
    }

    // No border agents here, because making the count work is hard.
    pub fn scaled_run(map: &Map, num_agents: usize) -> Scenario {
        Scenario {
            scenario_name: "scaled_run".to_string(),
            map_name: map.get_name().to_string(),
            seed_parked_cars: vec![SeedParkedCars {
                neighborhood: "_everywhere_".to_string(),
                cars_per_building: WeightedUsizeChoice {
                    weights: vec![5, 5],
                },
            }],
            spawn_over_time: vec![SpawnOverTime {
                num_agents: num_agents,
                start_time: Duration::ZERO,
                stop_time: Duration::seconds(5.0),
                start_from_neighborhood: "_everywhere_".to_string(),
                goal: OriginDestination::Neighborhood("_everywhere_".to_string()),
                percent_biking: 0.5,
                percent_use_transit: 0.5,
            }],
            border_spawn_over_time: Vec::new(),
            individ_trips: Vec::new(),
        }
    }

    pub fn rand_car(rng: &mut XorShiftRng) -> VehicleSpec {
        let length = Scenario::rand_dist(rng, MIN_CAR_LENGTH, MAX_CAR_LENGTH);
        VehicleSpec {
            vehicle_type: VehicleType::Car,
            length,
            max_speed: None,
        }
    }

    pub fn rand_bike(rng: &mut XorShiftRng) -> VehicleSpec {
        let max_speed = Some(Scenario::rand_speed(
            rng,
            Speed::miles_per_hour(8.0),
            Speed::miles_per_hour(10.0),
        ));
        VehicleSpec {
            vehicle_type: VehicleType::Bike,
            length: BIKE_LENGTH,
            max_speed,
        }
    }

    pub fn rand_dist(rng: &mut XorShiftRng, low: Distance, high: Distance) -> Distance {
        assert!(high > low);
        Distance::meters(rng.gen_range(low.inner_meters(), high.inner_meters()))
    }

    pub fn rand_speed(rng: &mut XorShiftRng, low: Speed, high: Speed) -> Speed {
        assert!(high > low);
        Speed::meters_per_second(rng.gen_range(
            low.inner_meters_per_second(),
            high.inner_meters_per_second(),
        ))
    }

    pub fn rand_ped_speed(rng: &mut XorShiftRng) -> Speed {
        // 2-3mph
        Scenario::rand_speed(
            rng,
            Speed::meters_per_second(0.894),
            Speed::meters_per_second(1.34),
        )
    }
}

impl SpawnOverTime {
    fn spawn_agent(
        &self,
        rng: &mut XorShiftRng,
        sim: &mut Sim,
        reserved_cars: &mut HashSet<CarID>,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        let spawn_time = rand_time(rng, self.start_time, self.stop_time);
        // Note that it's fine for agents to start/end at the same building. Later we might
        // want a better assignment of people per household, or workers per office building.
        let from_bldg = *neighborhoods[&self.start_from_neighborhood]
            .buildings
            .choose(rng)
            .unwrap();

        // What mode?
        if let Some(parked_car) = sim
            .get_parked_cars_by_owner(from_bldg)
            .into_iter()
            .find(|p| !reserved_cars.contains(&p.vehicle.id))
        {
            if let Some(goal) = self.goal.pick_driving_goal(
                vec![LaneType::Driving],
                map,
                &neighborhoods,
                rng,
                timer,
            ) {
                reserved_cars.insert(parked_car.vehicle.id);
                let spot = parked_car.spot;
                sim.schedule_trip(
                    spawn_time,
                    TripSpec::UsingParkedCar {
                        start: SidewalkSpot::building(from_bldg, map),
                        spot,
                        goal,
                        ped_speed: Scenario::rand_ped_speed(rng),
                    },
                    map,
                );
                return;
            }
        }

        if rng.gen_bool(self.percent_biking) {
            if let Some(goal) = self.goal.pick_driving_goal(
                vec![LaneType::Driving, LaneType::Biking],
                map,
                &neighborhoods,
                rng,
                timer,
            ) {
                let start_at = map.get_b(from_bldg).sidewalk();
                // TODO Just start biking on the other side of the street if the sidewalk
                // is on a one-way. Or at least warn.
                if map
                    .get_parent(start_at)
                    .sidewalk_to_bike(start_at)
                    .is_some()
                {
                    let ok = if let DrivingGoal::ParkNear(to_bldg) = goal {
                        let end_at = map.get_b(to_bldg).sidewalk();
                        map.get_parent(end_at).sidewalk_to_bike(end_at).is_some()
                            && start_at != end_at
                    } else {
                        true
                    };
                    if ok {
                        sim.schedule_trip(
                            spawn_time,
                            TripSpec::UsingBike {
                                start: SidewalkSpot::building(from_bldg, map),
                                vehicle: Scenario::rand_bike(rng),
                                goal,
                                ped_speed: Scenario::rand_ped_speed(rng),
                            },
                            map,
                        );
                        return;
                    }
                }
            }
        }

        if let Some(goal) = self.goal.pick_walking_goal(map, &neighborhoods, rng, timer) {
            let start_spot = SidewalkSpot::building(from_bldg, map);
            if start_spot == goal {
                timer.warn("Skipping walking trip between same two buildings".to_string());
                return;
            }

            if rng.gen_bool(self.percent_use_transit) {
                // TODO This throws away some work. It also sequentially does expensive
                // work right here.
                if let Some((stop1, stop2, route)) =
                    map.should_use_transit(start_spot.sidewalk_pos, goal.sidewalk_pos)
                {
                    sim.schedule_trip(
                        spawn_time,
                        TripSpec::UsingTransit {
                            start: start_spot,
                            route,
                            stop1,
                            stop2,
                            goal,
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                    );
                    return;
                }
            }

            sim.schedule_trip(
                spawn_time,
                TripSpec::JustWalking {
                    start: start_spot,
                    goal,
                    ped_speed: Scenario::rand_ped_speed(rng),
                },
                map,
            );
            return;
        }

        timer.warn(format!("Couldn't fulfill {:?} at all", self));
    }
}

impl BorderSpawnOverTime {
    fn spawn_peds(
        &self,
        rng: &mut XorShiftRng,
        sim: &mut Sim,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        if self.num_peds == 0 {
            return;
        }

        let start = if let Some(s) = SidewalkSpot::start_at_border(self.start_from_border, map) {
            s
        } else {
            timer.warn(format!(
                "Can't start_at_border for {} without sidewalk",
                self.start_from_border
            ));
            return;
        };

        for _ in 0..self.num_peds {
            let spawn_time = rand_time(rng, self.start_time, self.stop_time);
            if let Some(goal) = self.goal.pick_walking_goal(map, &neighborhoods, rng, timer) {
                if rng.gen_bool(self.percent_use_transit) {
                    // TODO This throws away some work. It also sequentially does expensive
                    // work right here.
                    if let Some((stop1, stop2, route)) =
                        map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
                    {
                        sim.schedule_trip(
                            spawn_time,
                            TripSpec::UsingTransit {
                                start: start.clone(),
                                route,
                                stop1,
                                stop2,
                                goal,
                                ped_speed: Scenario::rand_ped_speed(rng),
                            },
                            map,
                        );
                        continue;
                    }
                }

                sim.schedule_trip(
                    spawn_time,
                    TripSpec::JustWalking {
                        start: start.clone(),
                        goal,
                        ped_speed: Scenario::rand_ped_speed(rng),
                    },
                    map,
                );
            }
        }
    }

    fn spawn_cars(
        &self,
        rng: &mut XorShiftRng,
        sim: &mut Sim,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        if self.num_cars == 0 {
            return;
        }
        let starting_driving_lanes = map
            .get_i(self.start_from_border)
            .get_outgoing_lanes(map, LaneType::Driving);
        if starting_driving_lanes.is_empty() {
            timer.warn(format!(
                "Can't start car at border for {}",
                self.start_from_border
            ));
            return;
        }

        let lane_len = map.get_l(starting_driving_lanes[0]).length();
        if lane_len < MAX_CAR_LENGTH {
            timer.warn(format!(
                "Skipping {:?} because {} is only {}, too short to spawn cars",
                self, starting_driving_lanes[0], lane_len
            ));
            return;
        }
        for _ in 0..self.num_cars {
            let spawn_time = rand_time(rng, self.start_time, self.stop_time);
            if let Some(goal) = self.goal.pick_driving_goal(
                vec![LaneType::Driving],
                map,
                &neighborhoods,
                rng,
                timer,
            ) {
                let vehicle = Scenario::rand_car(rng);
                sim.schedule_trip(
                    spawn_time,
                    TripSpec::CarAppearing {
                        // TODO could pretty easily pick any lane here
                        start_pos: Position::new(starting_driving_lanes[0], vehicle.length),
                        vehicle_spec: vehicle,
                        goal,
                        ped_speed: Scenario::rand_ped_speed(rng),
                    },
                    map,
                );
            }
        }
    }

    fn spawn_bikes(
        &self,
        rng: &mut XorShiftRng,
        sim: &mut Sim,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        if self.num_bikes == 0 {
            return;
        }
        let mut starting_biking_lanes = map
            .get_i(self.start_from_border)
            .get_outgoing_lanes(map, LaneType::Biking);
        for l in map
            .get_i(self.start_from_border)
            .get_outgoing_lanes(map, LaneType::Driving)
        {
            if map.get_parent(l).supports_bikes() {
                starting_biking_lanes.push(l);
            }
        }
        if starting_biking_lanes.is_empty()
            || map.get_l(starting_biking_lanes[0]).length() < BIKE_LENGTH
        {
            timer.warn(format!(
                "Can't start bike at border for {}",
                self.start_from_border
            ));
            return;
        }

        for _ in 0..self.num_bikes {
            let spawn_time = rand_time(rng, self.start_time, self.stop_time);
            if let Some(goal) = self.goal.pick_driving_goal(
                vec![LaneType::Driving, LaneType::Biking],
                map,
                &neighborhoods,
                rng,
                timer,
            ) {
                let bike = Scenario::rand_bike(rng);
                sim.schedule_trip(
                    spawn_time,
                    TripSpec::CarAppearing {
                        start_pos: Position::new(starting_biking_lanes[0], bike.length),
                        vehicle_spec: bike,
                        goal,
                        ped_speed: Scenario::rand_ped_speed(rng),
                    },
                    map,
                );
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OriginDestination {
    Neighborhood(String),
    // TODO A serialized Scenario won't last well as the map changes...
    Border(IntersectionID),
}

impl OriginDestination {
    fn pick_driving_goal(
        &self,
        lane_types: Vec<LaneType>,
        map: &Map,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Neighborhood(ref n) => Some(DrivingGoal::ParkNear(
                *neighborhoods[n].buildings.choose(rng).unwrap(),
            )),
            OriginDestination::Border(i) => {
                let goal = DrivingGoal::end_at_border(*i, lane_types, map);
                if goal.is_none() {
                    timer.warn(format!(
                        "Can't spawn a car ending at border {}; no appropriate lanes there",
                        i
                    ));
                }
                goal
            }
        }
    }

    fn pick_walking_goal(
        &self,
        map: &Map,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<SidewalkSpot> {
        match self {
            OriginDestination::Neighborhood(ref n) => Some(SidewalkSpot::building(
                *neighborhoods[n].buildings.choose(rng).unwrap(),
                map,
            )),
            OriginDestination::Border(i) => {
                let goal = SidewalkSpot::end_at_border(*i, map);
                if goal.is_none() {
                    timer.warn(format!("Can't end_at_border for {} without a sidewalk", i));
                }
                goal
            }
        }
    }
}

fn seed_parked_cars(
    sim: &mut Sim,
    cars_per_building: &WeightedUsizeChoice,
    owner_buildings: &Vec<BuildingID>,
    neighborhoods_roads: &BTreeSet<RoadID>,
    base_rng: &mut XorShiftRng,
    map: &Map,
    timer: &mut Timer,
) {
    // Track the available parking spots per road, only for the roads in the appropriate
    // neighborhood.
    let mut total_spots = 0;
    let mut open_spots_per_road: HashMap<RoadID, Vec<ParkingSpot>> = HashMap::new();
    for id in neighborhoods_roads {
        let r = map.get_r(*id);
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for (lane, _) in r
            .children_forwards
            .iter()
            .chain(r.children_backwards.iter())
        {
            spots.extend(sim.get_free_spots(*lane));
        }
        total_spots += spots.len();
        spots.shuffle(&mut fork_rng(base_rng));
        open_spots_per_road.insert(r.id, spots);
    }

    let mut new_cars = 0;
    timer.start_iter("seed parked cars for buildings", owner_buildings.len());
    for b in owner_buildings {
        timer.next();
        for _ in 0..cars_per_building.sample(base_rng) {
            let mut forked_rng = fork_rng(base_rng);
            if let Some(spot) = find_spot_near_building(
                *b,
                &mut open_spots_per_road,
                neighborhoods_roads,
                map,
                timer,
            ) {
                sim.seed_parked_car(Scenario::rand_car(&mut forked_rng), spot, Some(*b));
                new_cars += 1;
            } else {
                // TODO This should be more critical, but neighborhoods can currently contain a
                // building, but not even its road, so this is inevitable.
                timer.warn(format!(
                    "No room to seed parked cars. {} total spots, {:?} of {} buildings requested, {} new cars so far. Searched from {}",
                    total_spots,
                    cars_per_building,
                    owner_buildings.len(),
                    new_cars,
                    b
                ));
            }
        }
    }

    timer.note(format!(
        "Seeded {} of {} parking spots with cars, leaving {} buildings without cars",
        new_cars,
        total_spots,
        owner_buildings.len() - new_cars
    ));
}

// Pick a parking spot for this building. If the building's road has a free spot, use it. If not,
// start BFSing out from the road in a deterministic way until finding a nearby road with an open
// spot.
fn find_spot_near_building(
    b: BuildingID,
    open_spots_per_road: &mut HashMap<RoadID, Vec<ParkingSpot>>,
    neighborhoods_roads: &BTreeSet<RoadID>,
    map: &Map,
    timer: &mut Timer,
) -> Option<ParkingSpot> {
    let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
    let mut visited: HashSet<RoadID> = HashSet::new();
    {
        let start = map.building_to_road(b).id;
        roads_queue.push_back(start);
        visited.insert(start);
    }

    loop {
        if roads_queue.is_empty() {
            timer.warn(format!(
                "Giving up looking for a free parking spot, searched {} roads of {}: {:?}",
                visited.len(),
                open_spots_per_road.len(),
                visited
            ));
        }
        let r = roads_queue.pop_front()?;
        if let Some(spots) = open_spots_per_road.get_mut(&r) {
            // TODO With some probability, skip this available spot and park farther away
            if !spots.is_empty() {
                return spots.pop();
            }
        }

        for next_r in map.get_next_roads(r).into_iter() {
            // Don't floodfill out of the neighborhood
            if !visited.contains(&next_r) && neighborhoods_roads.contains(&next_r) {
                roads_queue.push_back(next_r);
                visited.insert(next_r);
            }
        }
    }
}

fn rand_time(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    assert!(high > low);
    Duration::seconds(rng.gen_range(low.inner_seconds(), high.inner_seconds()))
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SpawnTrip {
    CarAppearing {
        depart: Duration,
        // TODO Replace start with building|border
        start: Position,
        start_bldg: Option<BuildingID>,
        goal: DrivingGoal,
        is_bike: bool,
    },
    UsingBike(Duration, SidewalkSpot, DrivingGoal),
    JustWalking(Duration, SidewalkSpot, SidewalkSpot),
    UsingTransit(
        Duration,
        SidewalkSpot,
        SidewalkSpot,
        BusRouteID,
        BusStopID,
        BusStopID,
    ),
}
