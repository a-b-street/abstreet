use crate::{DrivingGoal, IndividTrip, PersonID, PersonSpec, Scenario, SidewalkSpot, SpawnTrip};
use abstutil::Timer;
use geom::{Duration, Time};
use map_model::{BuildingID, DirectedRoadID, FullNeighborhoodInfo, Map, PathConstraints};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

// A way to generate Scenarios
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ScenarioGenerator {
    pub scenario_name: String,

    pub only_seed_buses: Option<BTreeSet<String>>,
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
    pub start_time: Time,
    pub stop_time: Time,
    pub start_from_neighborhood: String,
    pub goal: OriginDestination,
    pub percent_driving: f64,
    pub percent_biking: f64,
    pub percent_use_transit: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BorderSpawnOverTime {
    pub num_peds: usize,
    pub num_cars: usize,
    pub num_bikes: usize,
    pub percent_use_transit: f64,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_time: Time,
    pub stop_time: Time,
    pub start_from_border: DirectedRoadID,
    pub goal: OriginDestination,
}

impl ScenarioGenerator {
    // TODO may need to fork the RNG a bit more
    pub fn generate(&self, map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) -> Scenario {
        let mut scenario = Scenario::empty(map, &self.scenario_name);
        scenario.only_seed_buses = self.only_seed_buses.clone();

        timer.start(format!("Generating scenario {}", self.scenario_name));

        timer.start("load full neighborhood info");
        let neighborhoods = FullNeighborhoodInfo::load_all(map);
        timer.stop("load full neighborhood info");

        for s in &self.spawn_over_time {
            if !neighborhoods.contains_key(&s.start_from_neighborhood) {
                panic!("Neighborhood {} isn't defined", s.start_from_neighborhood);
            }

            timer.start_iter("SpawnOverTime each agent", s.num_agents);
            for _ in 0..s.num_agents {
                timer.next();
                s.spawn_agent(rng, &mut scenario, &neighborhoods, map, timer);
            }
        }

        timer.start_iter("BorderSpawnOverTime", self.border_spawn_over_time.len());
        for s in &self.border_spawn_over_time {
            timer.next();
            s.spawn_peds(rng, &mut scenario, &neighborhoods, map, timer);
            s.spawn_vehicles(
                s.num_cars,
                PathConstraints::Car,
                rng,
                &mut scenario,
                &neighborhoods,
                map,
                timer,
            );
            s.spawn_vehicles(
                s.num_bikes,
                PathConstraints::Bike,
                rng,
                &mut scenario,
                &neighborhoods,
                map,
                timer,
            );
        }

        timer.stop(format!("Generating scenario {}", self.scenario_name));
        scenario
    }

    pub fn small_run(map: &Map) -> ScenarioGenerator {
        let mut s = ScenarioGenerator {
            scenario_name: "small_run".to_string(),
            only_seed_buses: None,
            spawn_over_time: vec![SpawnOverTime {
                num_agents: 100,
                start_time: Time::START_OF_DAY,
                stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                start_from_neighborhood: "_everywhere_".to_string(),
                goal: OriginDestination::Neighborhood("_everywhere_".to_string()),
                percent_driving: 0.5,
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
                    start_time: Time::START_OF_DAY,
                    stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                    start_from_border: i.some_outgoing_road(map),
                    goal: OriginDestination::Neighborhood("_everywhere_".to_string()),
                    percent_use_transit: 0.5,
                })
                .collect(),
        };
        for i in map.all_outgoing_borders() {
            s.spawn_over_time.push(SpawnOverTime {
                num_agents: 10,
                start_time: Time::START_OF_DAY,
                stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                start_from_neighborhood: "_everywhere_".to_string(),
                goal: OriginDestination::EndOfRoad(i.some_incoming_road(map)),
                percent_driving: 0.5,
                percent_biking: 0.5,
                percent_use_transit: 0.5,
            });
        }
        s
    }

    pub fn empty(name: &str) -> ScenarioGenerator {
        ScenarioGenerator {
            scenario_name: name.to_string(),
            only_seed_buses: Some(BTreeSet::new()),
            spawn_over_time: Vec::new(),
            border_spawn_over_time: Vec::new(),
        }
    }

    // No border agents here, because making the count work is hard.
    pub fn scaled_run(num_agents: usize) -> ScenarioGenerator {
        ScenarioGenerator {
            scenario_name: "scaled_run".to_string(),
            only_seed_buses: Some(BTreeSet::new()),
            spawn_over_time: vec![SpawnOverTime {
                num_agents: num_agents,
                start_time: Time::START_OF_DAY,
                stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                start_from_neighborhood: "_everywhere_".to_string(),
                goal: OriginDestination::Neighborhood("_everywhere_".to_string()),
                percent_driving: 0.5,
                percent_biking: 0.5,
                percent_use_transit: 0.5,
            }],
            border_spawn_over_time: Vec::new(),
        }
    }
}

impl SpawnOverTime {
    fn spawn_agent(
        &self,
        rng: &mut XorShiftRng,
        scenario: &mut Scenario,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        let depart = rand_time(rng, self.start_time, self.stop_time);
        // Note that it's fine for agents to start/end at the same building. Later we might
        // want a better assignment of people per household, or workers per office building.
        let from_bldg = *neighborhoods[&self.start_from_neighborhood]
            .buildings
            .choose(rng)
            .unwrap();
        let id = PersonID(scenario.people.len());

        if rng.gen_bool(self.percent_driving) {
            if let Some(goal) =
                self.goal
                    .pick_driving_goal(PathConstraints::Car, map, &neighborhoods, rng, timer)
            {
                scenario.people.push(PersonSpec {
                    id,
                    orig_id: (0, 0),
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::UsingParkedCar(from_bldg, goal),
                    }],
                });
                return;
            }
        }

        let start_spot = SidewalkSpot::building(from_bldg, map);

        if rng.gen_bool(self.percent_biking) {
            if let Some(goal) =
                self.goal
                    .pick_driving_goal(PathConstraints::Bike, map, &neighborhoods, rng, timer)
            {
                scenario.people.push(PersonSpec {
                    id,
                    orig_id: (0, 0),
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::UsingBike(start_spot, goal),
                    }],
                });
                return;
            }
        }

        if let Some(goal) = self.goal.pick_walking_goal(map, &neighborhoods, rng, timer) {
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
                    scenario.people.push(PersonSpec {
                        id,
                        orig_id: (0, 0),
                        trips: vec![IndividTrip {
                            depart,
                            trip: SpawnTrip::UsingTransit(start_spot, goal, route, stop1, stop2),
                        }],
                    });
                    return;
                }
            }

            scenario.people.push(PersonSpec {
                id,
                orig_id: (0, 0),
                trips: vec![IndividTrip {
                    depart,
                    trip: SpawnTrip::JustWalking(start_spot, goal),
                }],
            });
            return;
        }

        timer.warn(format!("Couldn't fulfill {:?} at all", self));
    }
}

impl BorderSpawnOverTime {
    fn spawn_peds(
        &self,
        rng: &mut XorShiftRng,
        scenario: &mut Scenario,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        if self.num_peds == 0 {
            return;
        }

        let start = if let Some(s) =
            SidewalkSpot::start_at_border(self.start_from_border.src_i(map), map)
        {
            s
        } else {
            timer.warn(format!(
                "Can't start_at_border for {} without sidewalk",
                self.start_from_border
            ));
            return;
        };

        for _ in 0..self.num_peds {
            let depart = rand_time(rng, self.start_time, self.stop_time);
            let id = PersonID(scenario.people.len());
            if let Some(goal) = self.goal.pick_walking_goal(map, &neighborhoods, rng, timer) {
                if rng.gen_bool(self.percent_use_transit) {
                    // TODO This throws away some work. It also sequentially does expensive
                    // work right here.
                    if let Some((stop1, stop2, route)) =
                        map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
                    {
                        scenario.people.push(PersonSpec {
                            id,
                            orig_id: (0, 0),
                            trips: vec![IndividTrip {
                                depart,
                                trip: SpawnTrip::UsingTransit(
                                    start.clone(),
                                    goal,
                                    route,
                                    stop1,
                                    stop2,
                                ),
                            }],
                        });
                        continue;
                    }
                }

                scenario.people.push(PersonSpec {
                    id,
                    orig_id: (0, 0),
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::JustWalking(start.clone(), goal),
                    }],
                });
            }
        }
    }

    fn spawn_vehicles(
        &self,
        num: usize,
        constraints: PathConstraints,
        rng: &mut XorShiftRng,
        scenario: &mut Scenario,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        map: &Map,
        timer: &mut Timer,
    ) {
        for _ in 0..num {
            let depart = rand_time(rng, self.start_time, self.stop_time);
            if let Some(goal) =
                self.goal
                    .pick_driving_goal(constraints, map, &neighborhoods, rng, timer)
            {
                let id = PersonID(scenario.people.len());
                scenario.people.push(PersonSpec {
                    id,
                    orig_id: (0, 0),
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::FromBorder {
                            i: self.start_from_border.src_i(map),
                            goal,
                            is_bike: constraints == PathConstraints::Bike,
                        },
                    }],
                });
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OriginDestination {
    Neighborhood(String),
    EndOfRoad(DirectedRoadID),
    GotoBldg(BuildingID),
}

impl OriginDestination {
    fn pick_driving_goal(
        &self,
        constraints: PathConstraints,
        map: &Map,
        neighborhoods: &HashMap<String, FullNeighborhoodInfo>,
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Neighborhood(ref n) => Some(DrivingGoal::ParkNear(
                *neighborhoods[n].buildings.choose(rng).unwrap(),
            )),
            OriginDestination::GotoBldg(b) => Some(DrivingGoal::ParkNear(*b)),
            OriginDestination::EndOfRoad(dr) => {
                let goal = DrivingGoal::end_at_border(*dr, constraints, map);
                if goal.is_none() {
                    timer.warn(format!(
                        "Can't spawn a {:?} ending at border {}; no appropriate lanes there",
                        constraints, dr
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
            OriginDestination::EndOfRoad(dr) => {
                let goal = SidewalkSpot::end_at_border(dr.dst_i(map), map);
                if goal.is_none() {
                    timer.warn(format!("Can't end_at_border for {} without a sidewalk", dr));
                }
                goal
            }
            OriginDestination::GotoBldg(b) => Some(SidewalkSpot::building(*b, map)),
        }
    }
}

fn rand_time(rng: &mut XorShiftRng, low: Time, high: Time) -> Time {
    assert!(high > low);
    Time::START_OF_DAY + Duration::seconds(rng.gen_range(low.inner_seconds(), high.inner_seconds()))
}
