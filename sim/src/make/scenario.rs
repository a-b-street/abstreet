use crate::driving::DrivingGoal;
use crate::kinematics;
use crate::walking::SidewalkSpot;
use crate::{CarID, Sim, Tick};
use abstutil;
use abstutil::{Timer, WeightedUsizeChoice};
use geom::Distance;
use map_model::{FullNeighborhoodInfo, IntersectionID, LaneType, Map, Pathfinder, Position};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    pub percent_use_transit: f64,
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
    pub percent_use_transit: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SeedParkedCars {
    pub neighborhood: String,
    pub cars_per_building: WeightedUsizeChoice,
}

impl Scenario {
    pub fn describe(&self) -> Vec<String> {
        abstutil::to_json(self)
            .split('\n')
            .map(|s| s.to_string())
            .collect()
    }

    // TODO may need to fork the RNG a bit more
    pub fn instantiate(&self, sim: &mut Sim, map: &Map, timer: &mut Timer) {
        timer.start(&format!("Instantiating {}", self.scenario_name));
        assert!(sim.time == Tick::zero());

        timer.start("load full neighborhood info");
        let neighborhoods = FullNeighborhoodInfo::load_all(map);
        timer.stop("load full neighborhood info");

        for s in &self.seed_parked_cars {
            if !neighborhoods.contains_key(&s.neighborhood) {
                panic!("Neighborhood {} isn't defined", s.neighborhood);
            }

            sim.seed_parked_cars(
                &neighborhoods[&s.neighborhood].buildings,
                &neighborhoods[&s.neighborhood].roads,
                &s.cars_per_building,
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
                let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                // Note that it's fine for agents to start/end at the same building. Later we might
                // want a better assignment of people per household, or workers per office building.
                let from_bldg = *neighborhoods[&s.start_from_neighborhood]
                    .buildings
                    .choose(&mut sim.rng)
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
                            .pick_driving_goal(map, &neighborhoods, &mut sim.rng, timer)
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
                            .pick_biking_goal(map, &neighborhoods, &mut sim.rng, timer)
                    {
                        let skip = if let DrivingGoal::ParkNear(to_bldg) = goal {
                            map.get_b(to_bldg).sidewalk() == map.get_b(from_bldg).sidewalk()
                        } else {
                            false
                        };

                        if !skip {
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
                    }
                } else if let Some(goal) =
                    s.goal
                        .pick_walking_goal(map, &neighborhoods, &mut sim.rng, timer)
                {
                    let start_spot = SidewalkSpot::building(from_bldg, map);

                    if sim.rng.gen_bool(s.percent_use_transit) {
                        // TODO This throws away some work. It also sequentially does expensive
                        // work right here.
                        if let Some((stop1, stop2, route)) = Pathfinder::should_use_transit(
                            map,
                            start_spot.sidewalk_pos,
                            goal.sidewalk_pos,
                        ) {
                            sim.spawner.start_trip_using_bus(
                                spawn_time,
                                map,
                                start_spot,
                                goal,
                                route,
                                stop1,
                                stop2,
                                &mut sim.trips_state,
                            );
                            continue;
                        }
                    }

                    sim.spawner.start_trip_just_walking(
                        spawn_time,
                        start_spot,
                        goal,
                        &mut sim.trips_state,
                    );
                }
            }
        }

        timer.start_iter("BorderSpawnOverTime", self.border_spawn_over_time.len());
        for s in &self.border_spawn_over_time {
            timer.next();
            if let Some(start) = SidewalkSpot::start_at_border(s.start_from_border, map) {
                for _ in 0..s.num_peds {
                    let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                    if let Some(goal) =
                        s.goal
                            .pick_walking_goal(map, &neighborhoods, &mut sim.rng, timer)
                    {
                        if sim.rng.gen_bool(s.percent_use_transit) {
                            // TODO This throws away some work. It also sequentially does expensive
                            // work right here.
                            if let Some((stop1, stop2, route)) = Pathfinder::should_use_transit(
                                map,
                                start.sidewalk_pos,
                                goal.sidewalk_pos,
                            ) {
                                sim.spawner.start_trip_using_bus(
                                    spawn_time,
                                    map,
                                    start.clone(),
                                    goal,
                                    route,
                                    stop1,
                                    stop2,
                                    &mut sim.trips_state,
                                );
                                continue;
                            }
                        }

                        sim.spawner.start_trip_just_walking(
                            spawn_time,
                            start.clone(),
                            goal,
                            &mut sim.trips_state,
                        );
                    }
                }
            } else if s.num_peds > 0 {
                timer.warn(format!(
                    "Can't start_at_border for {} without sidewalk",
                    s.start_from_border
                ));
            }

            let starting_driving_lanes = map
                .get_i(s.start_from_border)
                .get_outgoing_lanes(map, LaneType::Driving);
            if !starting_driving_lanes.is_empty() {
                let lane_len = map.get_l(starting_driving_lanes[0]).length();
                if lane_len < kinematics::MAX_CAR_LENGTH {
                    timer.warn(format!(
                        "Skipping {:?} because {} is only {}, too short to spawn cars",
                        s, starting_driving_lanes[0], lane_len
                    ));
                } else {
                    for _ in 0..s.num_cars {
                        let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                        if let Some(goal) =
                            s.goal
                                .pick_driving_goal(map, &neighborhoods, &mut sim.rng, timer)
                        {
                            sim.spawner.start_trip_with_car_appearing(
                                spawn_time,
                                map,
                                // TODO could pretty easily pick any lane here
                                Position::new(starting_driving_lanes[0], Distance::ZERO),
                                goal,
                                &mut sim.trips_state,
                                &mut sim.rng,
                            );
                        }
                    }
                }
            } else if s.num_cars > 0 {
                timer.warn(format!(
                    "Can't start car at border for {}",
                    s.start_from_border
                ));
            }

            let mut starting_biking_lanes = map
                .get_i(s.start_from_border)
                .get_outgoing_lanes(map, LaneType::Biking);
            for l in starting_driving_lanes {
                if map.get_parent(l).supports_bikes() {
                    starting_biking_lanes.push(l);
                }
            }
            if !starting_biking_lanes.is_empty() {
                for _ in 0..s.num_bikes {
                    let spawn_time = Tick::uniform(s.start_tick, s.stop_tick, &mut sim.rng);
                    if let Some(goal) =
                        s.goal
                            .pick_biking_goal(map, &neighborhoods, &mut sim.rng, timer)
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
            } else if s.num_bikes > 0 {
                timer.warn(format!(
                    "Can't start bike at border for {}",
                    s.start_from_border
                ));
            }
        }

        timer.stop(&format!("Instantiating {}", self.scenario_name));
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
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Neighborhood(ref n) => Some(DrivingGoal::ParkNear(
                *neighborhoods[n].buildings.choose(rng).unwrap(),
            )),
            OriginDestination::Border(i) => {
                let lanes = map.get_i(*i).get_incoming_lanes(map, LaneType::Driving);
                if lanes.is_empty() {
                    timer.warn(format!(
                        "Can't spawn a car ending at border {}; no driving lane there",
                        i
                    ));
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
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Neighborhood(ref n) => Some(DrivingGoal::ParkNear(
                *neighborhoods[n].buildings.choose(rng).unwrap(),
            )),
            OriginDestination::Border(i) => {
                let mut lanes = map.get_i(*i).get_incoming_lanes(map, LaneType::Biking);
                lanes.extend(map.get_i(*i).get_incoming_lanes(map, LaneType::Driving));
                if lanes.is_empty() {
                    timer.warn(format!(
                        "Can't spawn a bike ending at border {}; no biking or driving lane there",
                        i
                    ));
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
