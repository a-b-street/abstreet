//! This is a much more primitive way to randomly generate trips. activity_model.rs has something
//! more realistic.

use std::collections::BTreeSet;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::{Duration, Time};
use map_model::{IntersectionID, Map};

use crate::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};

// TODO This can be simplified dramatically.

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
    pub goal: Option<TripEndpoint>,
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
    pub start_from_border: IntersectionID,
    pub goal: Option<TripEndpoint>,
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
                s.spawn_agent(rng, &mut scenario, map);
            }
        }

        timer.start_iter("BorderSpawnOverTime", self.border_spawn_over_time.len());
        for s in &self.border_spawn_over_time {
            timer.next();
            for _ in 0..s.num_peds {
                let mode = if rng.gen_bool(s.percent_use_transit) {
                    TripMode::Transit
                } else {
                    TripMode::Walk
                };
                s.spawn(rng, &mut scenario, mode, map);
            }
            for _ in 0..s.num_cars {
                s.spawn(rng, &mut scenario, TripMode::Drive, map);
            }
            for _ in 0..s.num_bikes {
                s.spawn(rng, &mut scenario, TripMode::Bike, map);
            }
        }

        timer.stop(format!("Generating scenario {}", self.scenario_name));
        scenario.remove_weird_schedules()
    }

    pub fn small_run(map: &Map) -> ScenarioGenerator {
        let mut s = ScenarioGenerator {
            scenario_name: "small_run".to_string(),
            only_seed_buses: None,
            spawn_over_time: vec![SpawnOverTime {
                num_agents: 100,
                start_time: Time::START_OF_DAY,
                stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                goal: None,
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
                    start_from_border: i.id,
                    goal: None,
                    percent_use_transit: 0.5,
                })
                .collect(),
        };
        for i in map.all_outgoing_borders() {
            s.spawn_over_time.push(SpawnOverTime {
                num_agents: 10,
                start_time: Time::START_OF_DAY,
                stop_time: Time::START_OF_DAY + Duration::seconds(5.0),
                goal: Some(TripEndpoint::Border(i.id)),
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
}

impl SpawnOverTime {
    fn spawn_agent(&self, rng: &mut XorShiftRng, scenario: &mut Scenario, map: &Map) {
        let depart = rand_time(rng, self.start_time, self.stop_time);
        // Note that it's fine for agents to start/end at the same building. Later we might
        // want a better assignment of people per household, or workers per office building.
        let from_bldg = map.all_buildings().choose(rng).unwrap().id;
        let mode = if rng.gen_bool(self.percent_driving) {
            TripMode::Drive
        } else if rng.gen_bool(self.percent_biking) {
            TripMode::Bike
        } else if rng.gen_bool(self.percent_use_transit) {
            TripMode::Transit
        } else {
            TripMode::Walk
        };
        scenario.people.push(PersonSpec {
            orig_id: None,
            origin: TripEndpoint::Bldg(from_bldg),
            trips: vec![IndividTrip::new(
                depart,
                TripPurpose::Shopping,
                self.goal.clone().unwrap_or_else(|| {
                    TripEndpoint::Bldg(map.all_buildings().choose(rng).unwrap().id)
                }),
                mode,
            )],
        });
    }
}

impl BorderSpawnOverTime {
    fn spawn(&self, rng: &mut XorShiftRng, scenario: &mut Scenario, mode: TripMode, map: &Map) {
        let depart = rand_time(rng, self.start_time, self.stop_time);
        scenario.people.push(PersonSpec {
            orig_id: None,
            origin: TripEndpoint::Border(self.start_from_border),
            trips: vec![IndividTrip::new(
                depart,
                TripPurpose::Shopping,
                self.goal.clone().unwrap_or_else(|| {
                    TripEndpoint::Bldg(map.all_buildings().choose(rng).unwrap().id)
                }),
                mode,
            )],
        });
    }
}

fn rand_time(rng: &mut XorShiftRng, low: Time, high: Time) -> Time {
    assert!(high > low);
    Time::START_OF_DAY + Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}
