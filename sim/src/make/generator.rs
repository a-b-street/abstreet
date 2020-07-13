use crate::{
    DrivingGoal, IndividTrip, PersonID, PersonSpec, Scenario, SidewalkSpot, SpawnTrip,
    TripEndpoint, TripMode,
};
use abstutil::Timer;
use geom::{Distance, Duration, Time};
use map_model::{BuildingID, BuildingType, DirectedRoadID, Map, PathConstraints, PathRequest};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

        for s in &self.spawn_over_time {
            timer.start_iter("SpawnOverTime each agent", s.num_agents);
            for _ in 0..s.num_agents {
                timer.next();
                s.spawn_agent(rng, &mut scenario, map, timer);
            }
        }

        timer.start_iter("BorderSpawnOverTime", self.border_spawn_over_time.len());
        for s in &self.border_spawn_over_time {
            timer.next();
            s.spawn_peds(rng, &mut scenario, map, timer);
            s.spawn_vehicles(
                s.num_cars,
                PathConstraints::Car,
                rng,
                &mut scenario,
                map,
                timer,
            );
            s.spawn_vehicles(
                s.num_bikes,
                PathConstraints::Bike,
                rng,
                &mut scenario,
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
                goal: OriginDestination::Anywhere,
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
                    start_from_border: i.some_outgoing_road(map).unwrap(),
                    goal: OriginDestination::Anywhere,
                    percent_use_transit: 0.5,
                })
                .collect(),
        };
        for i in map.all_outgoing_borders() {
            s.spawn_over_time.push(SpawnOverTime {
                num_agents: 10,
                start_time: Time::START_OF_DAY,
                stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                goal: OriginDestination::EndOfRoad(i.some_incoming_road(map).unwrap()),
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
                goal: OriginDestination::Anywhere,
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
        map: &Map,
        timer: &mut Timer,
    ) {
        let depart = rand_time(rng, self.start_time, self.stop_time);
        // Note that it's fine for agents to start/end at the same building. Later we might
        // want a better assignment of people per household, or workers per office building.
        let from_bldg = map.all_buildings().choose(rng).unwrap().id;
        let id = PersonID(scenario.people.len());

        if rng.gen_bool(self.percent_driving) {
            if let Some(goal) = self
                .goal
                .pick_driving_goal(PathConstraints::Car, map, rng, timer)
            {
                scenario.people.push(PersonSpec {
                    id,
                    orig_id: None,
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::UsingParkedCar(from_bldg, goal),
                        cancelled: false,
                    }],
                });
                return;
            }
        }

        let start_spot = SidewalkSpot::building(from_bldg, map);

        if rng.gen_bool(self.percent_biking) {
            if let Some(goal) = self
                .goal
                .pick_driving_goal(PathConstraints::Bike, map, rng, timer)
            {
                scenario.people.push(PersonSpec {
                    id,
                    orig_id: None,
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::UsingBike(start_spot, goal),
                        cancelled: false,
                    }],
                });
                return;
            }
        }

        if let Some(goal) = self.goal.pick_walking_goal(map, rng, timer) {
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
                        orig_id: None,
                        trips: vec![IndividTrip {
                            depart,
                            trip: SpawnTrip::UsingTransit(start_spot, goal, route, stop1, stop2),
                            cancelled: false,
                        }],
                    });
                    return;
                }
            }

            scenario.people.push(PersonSpec {
                id,
                orig_id: None,
                trips: vec![IndividTrip {
                    depart,
                    trip: SpawnTrip::JustWalking(start_spot, goal),
                    cancelled: false,
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
        map: &Map,
        timer: &mut Timer,
    ) {
        if self.num_peds == 0 {
            return;
        }

        let start = if let Some(s) =
            SidewalkSpot::start_at_border(self.start_from_border.src_i(map), None, map)
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
            if let Some(goal) = self.goal.pick_walking_goal(map, rng, timer) {
                if rng.gen_bool(self.percent_use_transit) {
                    // TODO This throws away some work. It also sequentially does expensive
                    // work right here.
                    if let Some((stop1, stop2, route)) =
                        map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
                    {
                        scenario.people.push(PersonSpec {
                            id,
                            orig_id: None,
                            trips: vec![IndividTrip {
                                depart,
                                trip: SpawnTrip::UsingTransit(
                                    start.clone(),
                                    goal,
                                    route,
                                    stop1,
                                    stop2,
                                ),
                                cancelled: false,
                            }],
                        });
                        continue;
                    }
                }

                scenario.people.push(PersonSpec {
                    id,
                    orig_id: None,
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::JustWalking(start.clone(), goal),
                        cancelled: false,
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
        map: &Map,
        timer: &mut Timer,
    ) {
        for _ in 0..num {
            let depart = rand_time(rng, self.start_time, self.stop_time);
            if let Some(goal) = self.goal.pick_driving_goal(constraints, map, rng, timer) {
                let id = PersonID(scenario.people.len());
                scenario.people.push(PersonSpec {
                    id,
                    orig_id: None,
                    trips: vec![IndividTrip {
                        depart,
                        trip: SpawnTrip::FromBorder {
                            dr: self.start_from_border,
                            goal,
                            is_bike: constraints == PathConstraints::Bike,
                            origin: None,
                        },
                        cancelled: false,
                    }],
                });
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OriginDestination {
    Anywhere,
    EndOfRoad(DirectedRoadID),
    GotoBldg(BuildingID),
}

impl OriginDestination {
    fn pick_driving_goal(
        &self,
        constraints: PathConstraints,
        map: &Map,
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<DrivingGoal> {
        match self {
            OriginDestination::Anywhere => Some(DrivingGoal::ParkNear(
                map.all_buildings().choose(rng).unwrap().id,
            )),
            OriginDestination::GotoBldg(b) => Some(DrivingGoal::ParkNear(*b)),
            OriginDestination::EndOfRoad(dr) => {
                let goal = DrivingGoal::end_at_border(*dr, constraints, None, map);
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
        rng: &mut XorShiftRng,
        timer: &mut Timer,
    ) -> Option<SidewalkSpot> {
        match self {
            OriginDestination::Anywhere => Some(SidewalkSpot::building(
                map.all_buildings().choose(rng).unwrap().id,
                map,
            )),
            OriginDestination::EndOfRoad(dr) => {
                let goal = SidewalkSpot::end_at_border(dr.dst_i(map), None, map);
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

impl ScenarioGenerator {
    // Designed in https://github.com/dabreegster/abstreet/issues/154
    pub fn proletariat_robot(map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) -> Scenario {
        let mut residences: Vec<(BuildingID, usize)> = Vec::new();
        let mut workplaces: Vec<BuildingID> = Vec::new();
        let mut total_ppl = 0;
        for b in map.all_buildings() {
            match b.bldg_type {
                BuildingType::Residential(num_ppl) => {
                    residences.push((b.id, num_ppl));
                    total_ppl += num_ppl;
                }
                BuildingType::ResidentialCommercial(num_ppl) => {
                    residences.push((b.id, num_ppl));
                    total_ppl += num_ppl;
                    workplaces.push(b.id);
                }
                BuildingType::Commercial => {
                    workplaces.push(b.id);
                }
                BuildingType::Empty => {}
            }
        }

        let mut s = Scenario::empty(map, "random people going to/from work");
        s.only_seed_buses = None;
        timer.start_iter("create people", total_ppl);
        for (home, num_ppl) in residences {
            for _ in 0..num_ppl {
                timer.next();
                // Make a person going from their home to a random workplace, then back again later.
                let work = *workplaces.choose(rng).unwrap();
                // Decide mode based on walking distance.
                let dist = if let Some(path) = map.pathfind(PathRequest {
                    start: map.get_b(home).front_path.sidewalk,
                    end: map.get_b(work).front_path.sidewalk,
                    constraints: PathConstraints::Pedestrian,
                }) {
                    path.total_length()
                } else {
                    // Woops, the buildings aren't connected. Probably a bug in importing. Just skip
                    // this person.
                    continue;
                };
                if home.0 == work.0 {
                    // working and living in the same building
                    continue;
                }
                // Longer trips will mostly drive of the time, remaining will attempt
                // transit (falling back to a very long walk), with some small number of people cycling.
                // TODO Make this probabilistic
                // TODO - do not select based on distance but select one that is fastest/best in the given situation
                // excellent bus connection / plenty of parking / cycleways / suitable rail connection
                // all strongly influence selected mode of transport, distance is not the sole influence
                let mode = if dist < Distance::miles(0.5) {
                    TripMode::Walk
                } else if dist < Distance::miles(2.0) {
                    if rng.gen_bool(0.3) {
                        // 30%
                        TripMode::Transit
                    } else if rng.gen_bool(0.8) {
                        // 0.7 * 0.8 = 56%
                        TripMode::Drive
                    } else {
                        // 14%
                        TripMode::Bike
                    }
                } else if rng.gen_bool(0.005) {
                    // low chance for really, really dedicated cyclists
                    TripMode::Bike
                } else if rng.gen_bool(0.7) {
                    TripMode::Drive
                } else {
                    TripMode::Transit
                };

                // TODO This will cause a single morning and afternoon rush. Outside of these times,
                // it'll be really quiet. Probably want a normal distribution centered around these
                // peak times, but with a long tail.
                let depart_am = rand_time(
                    rng,
                    Time::START_OF_DAY + Duration::hours(7),
                    Time::START_OF_DAY + Duration::hours(10),
                );
                let depart_pm = rand_time(
                    rng,
                    Time::START_OF_DAY + Duration::hours(17),
                    Time::START_OF_DAY + Duration::hours(19),
                );

                let (goto_work, return_home) = match (
                    SpawnTrip::new(
                        TripEndpoint::Bldg(home),
                        TripEndpoint::Bldg(work),
                        mode,
                        map,
                    ),
                    SpawnTrip::new(
                        TripEndpoint::Bldg(work),
                        TripEndpoint::Bldg(home),
                        mode,
                        map,
                    ),
                ) {
                    (Some(t1), Some(t2)) => (t1, t2),
                    // Skip the person if either trip can't be created.
                    _ => continue,
                };

                s.people.push(PersonSpec {
                    id: PersonID(s.people.len()),
                    orig_id: None,
                    trips: vec![
                        IndividTrip {
                            depart: depart_am,
                            trip: goto_work,
                            cancelled: false,
                        },
                        IndividTrip {
                            depart: depart_pm,
                            trip: return_home,
                            cancelled: false,
                        },
                    ],
                });
            }
        }
        s
    }
}
