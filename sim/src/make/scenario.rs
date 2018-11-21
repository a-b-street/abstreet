use abstutil;
use abstutil::WeightedUsizeChoice;
use driving::DrivingGoal;
use map_model::{BuildingID, IntersectionID, LaneType, Map, RoadID};
use rand::{Rng, XorShiftRng};
use std::collections::{BTreeSet, HashMap, HashSet};
use walking::SidewalkSpot;
use {CarID, Neighborhood, Sim, Tick};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: String,

    pub seed_parked_cars: Vec<SeedParkedCars>,
    pub spawn_over_time: Vec<SpawnOverTime>,
    pub border_spawn_over_time: Vec<BorderSpawnOverTime>,
}

// SpawnOverTime and BorderSpawnOverTime should be kept separate. Agents in SpawnOverTime pick
// their mode (use a car, walk, bus) based on the situation. When spawning directly a border,
// agents have to start as a car or pedestrian already.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SpawnOverTime {
    pub num_agents: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_tick: Tick,
    pub stop_tick: Tick,
    pub start_from_neighborhood: String,
    pub goal: OriginDestination,
    pub percent_biking: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BorderSpawnOverTime {
    pub num_peds: usize,
    pub num_cars: usize,
    pub num_bikes: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_tick: Tick,
    pub stop_tick: Tick,
    // TODO A serialized Scenario won't last well as the map changes...
    pub start_from_border: IntersectionID,
    pub goal: OriginDestination,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SeedParkedCars {
    pub neighborhood: String,
    pub cars_per_building: WeightedUsizeChoice,
}

impl Scenario {
    pub fn describe(&self) -> Vec<String> {
        abstutil::to_json(self)
            .split("\n")
            .map(|s| s.to_string())
            .collect()
    }

    // TODO may need to fork the RNG a bit more
    pub fn instantiate(&self, sim: &mut Sim, map: &Map) {
        info!("Instantiating {}", self.scenario_name);
        assert!(sim.time == Tick::zero());

        let gps_bounds = map.get_gps_bounds();
        let mut neighborhoods: HashMap<String, Neighborhood> =
            Neighborhood::load_all(&self.map_name, gps_bounds)
                .into_iter()
                .collect();
        neighborhoods.insert(
            "_everywhere_".to_string(),
            Neighborhood::make_everywhere(map),
        );

        let mut bldgs_per_neighborhood: HashMap<String, Vec<BuildingID>> = HashMap::new();
        for (name, neighborhood) in &neighborhoods {
            bldgs_per_neighborhood
                .insert(name.to_string(), neighborhood.find_matching_buildings(map));
        }
        let mut roads_per_neighborhood: HashMap<String, BTreeSet<RoadID>> = HashMap::new();
        for (name, neighborhood) in &neighborhoods {
            roads_per_neighborhood.insert(name.to_string(), neighborhood.find_matching_roads(map));
        }

        for s in &self.seed_parked_cars {
            if !neighborhoods.contains_key(&s.neighborhood) {
                panic!("Neighborhood {} isn't defined", s.neighborhood);
            }

            sim.seed_parked_cars(
                &bldgs_per_neighborhood[&s.neighborhood],
                &roads_per_neighborhood[&s.neighborhood],
                &s.cars_per_building,
                map,
            );
        }

        // Don't let two pedestrians starting from one building use the same car.
        let mut reserved_cars: HashSet<CarID> = HashSet::new();
        for s in &self.spawn_over_time {
            if !neighborhoods.contains_key(&s.start_from_neighborhood) {
                panic!("Neighborhood {} isn't defined", s.start_from_neighborhood);
            }

            for _ in 0..s.num_agents {
                let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                // Note that it's fine for agents to start/end at the same building. Later we might
                // want a better assignment of people per household, or workers per office building.
                let from_bldg = *sim
                    .rng
                    .choose(&bldgs_per_neighborhood[&s.start_from_neighborhood])
                    .unwrap();

                // What mode?
                if let Some(parked_car) = sim
                    .parking_state
                    .get_parked_cars_by_owner(from_bldg)
                    .into_iter()
                    .find(|p| !reserved_cars.contains(&p.car))
                {
                    if let Some(goal) =
                        s.goal
                            .pick_driving_goal(map, &bldgs_per_neighborhood, &mut sim.rng)
                    {
                        reserved_cars.insert(parked_car.car);
                        sim.spawner.start_trip_using_parked_car(
                            spawn_time,
                            map,
                            parked_car.clone(),
                            &sim.parking_state,
                            from_bldg,
                            goal,
                            &mut sim.trips_state,
                        );
                    }
                } else if sim.rng.gen_bool(s.percent_biking) {
                    if let Some(goal) =
                        s.goal
                            .pick_biking_goal(map, &bldgs_per_neighborhood, &mut sim.rng)
                    {
                        sim.spawner.start_trip_using_bike(
                            spawn_time,
                            map,
                            from_bldg,
                            goal,
                            &mut sim.trips_state,
                            // TODO, like the biking goal could exist here or not based on border
                            // map edits. so fork before this choice is made?
                            &mut sim.rng,
                        );
                    }
                } else {
                    if let Some(goal) =
                        s.goal
                            .pick_walking_goal(map, &bldgs_per_neighborhood, &mut sim.rng)
                    {
                        sim.spawner.start_trip_just_walking(
                            spawn_time,
                            SidewalkSpot::building(from_bldg, map),
                            goal,
                            &mut sim.trips_state,
                        );
                    }
                }
            }
        }

        for s in &self.border_spawn_over_time {
            if let Some(start) = SidewalkSpot::start_at_border(s.start_from_border, map) {
                for _ in 0..s.num_peds {
                    let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                    if let Some(goal) =
                        s.goal
                            .pick_walking_goal(map, &bldgs_per_neighborhood, &mut sim.rng)
                    {
                        sim.spawner.start_trip_just_walking(
                            spawn_time,
                            start.clone(),
                            goal,
                            &mut sim.trips_state,
                        );
                    }
                }
            } else {
                warn!(
                    "Can't start_at_border for {} without sidewalk",
                    s.start_from_border
                );
            }

            let starting_driving_lanes = map
                .get_i(s.start_from_border)
                .get_outgoing_lanes(map, LaneType::Driving);
            if !starting_driving_lanes.is_empty() {
                for _ in 0..s.num_cars {
                    let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                    if let Some(goal) =
                        s.goal
                            .pick_driving_goal(map, &bldgs_per_neighborhood, &mut sim.rng)
                    {
                        sim.spawner.start_trip_with_car_at_border(
                            spawn_time,
                            map,
                            // TODO could pretty easily pick any lane here
                            starting_driving_lanes[0],
                            goal,
                            &mut sim.trips_state,
                            &mut sim.rng,
                        );
                    }
                }
            } else {
                warn!("Can't start car at border for {}", s.start_from_border);
            }

            let mut starting_biking_lanes = map
                .get_i(s.start_from_border)
                .get_outgoing_lanes(map, LaneType::Biking);
            starting_biking_lanes.extend(starting_driving_lanes);
            if !starting_biking_lanes.is_empty() {
                for _ in 0..s.num_bikes {
                    let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                    if let Some(goal) =
                        s.goal
                            .pick_biking_goal(map, &bldgs_per_neighborhood, &mut sim.rng)
                    {
                        sim.spawner.start_trip_with_bike_at_border(
                            spawn_time,
                            map,
                            starting_biking_lanes[0],
                            goal,
                            &mut sim.trips_state,
                            &mut sim.rng,
                        );
                    }
                }
            } else {
                warn!("Can't start bike at border for {}", s.start_from_border);
            }
        }
    }

    pub fn save(&self) {
        abstutil::save_object("scenarios", &self.map_name, &self.scenario_name, self);
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
        map: &Map,
        bldgs_per_neighborhood: &HashMap<String, Vec<BuildingID>>,
        rng: &mut XorShiftRng,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Neighborhood(ref n) => {
                if let Some(bldgs) = bldgs_per_neighborhood.get(n) {
                    Some(DrivingGoal::ParkNear(*rng.choose(bldgs).unwrap()))
                } else {
                    panic!("Neighborhood {} isn't defined", n);
                }
            }
            OriginDestination::Border(i) => {
                let lanes = map.get_i(*i).get_incoming_lanes(map, LaneType::Driving);
                if lanes.is_empty() {
                    warn!(
                        "Can't spawn a car ending at border {}; no driving lane there",
                        i
                    );
                    None
                } else {
                    // TODO ideally could use any
                    Some(DrivingGoal::Border(*i, lanes[0]))
                }
            }
        }
    }

    // TODO nearly a copy of pick_driving_goal! Ew
    fn pick_biking_goal(
        &self,
        map: &Map,
        bldgs_per_neighborhood: &HashMap<String, Vec<BuildingID>>,
        rng: &mut XorShiftRng,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Neighborhood(ref n) => {
                if let Some(bldgs) = bldgs_per_neighborhood.get(n) {
                    Some(DrivingGoal::ParkNear(*rng.choose(bldgs).unwrap()))
                } else {
                    panic!("Neighborhood {} isn't defined", n);
                }
            }
            OriginDestination::Border(i) => {
                let mut lanes = map.get_i(*i).get_incoming_lanes(map, LaneType::Biking);
                lanes.extend(map.get_i(*i).get_incoming_lanes(map, LaneType::Driving));
                if lanes.is_empty() {
                    warn!(
                        "Can't spawn a bike ending at border {}; no biking or driving lane there",
                        i
                    );
                    None
                } else {
                    Some(DrivingGoal::Border(*i, lanes[0]))
                }
            }
        }
    }

    fn pick_walking_goal(
        &self,
        map: &Map,
        bldgs_per_neighborhood: &HashMap<String, Vec<BuildingID>>,
        rng: &mut XorShiftRng,
    ) -> Option<SidewalkSpot> {
        match self {
            OriginDestination::Neighborhood(ref n) => {
                if let Some(bldgs) = bldgs_per_neighborhood.get(n) {
                    Some(SidewalkSpot::building(*rng.choose(bldgs).unwrap(), map))
                } else {
                    panic!("Neighborhood {} isn't defined", n);
                }
            }
            OriginDestination::Border(i) => {
                let goal = SidewalkSpot::end_at_border(*i, map);
                if goal.is_none() {
                    warn!("Can't end_at_border for {} without a sidewalk", i);
                }
                goal
            }
        }
    }
}
