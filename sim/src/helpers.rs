use crate::driving::DrivingGoal;
use crate::walking::SidewalkSpot;
use crate::{
    BorderSpawnOverTime, CarID, Event, OriginDestination, PedestrianID, Scenario, SeedParkedCars,
    Sim, SpawnOverTime, Tick,
};
use abstutil::{Timer, WeightedUsizeChoice};
use geom::Duration;
use map_model::{
    BuildingID, BusRoute, BusRouteID, BusStopID, IntersectionID, LaneID, LaneType, Map, Position,
    RoadID,
};
use std::collections::{BTreeSet, VecDeque};
use std::panic;

// Helpers to run the sim
impl Sim {
    // TODO share the helpers for spawning specific parking spots and stuff?

    pub fn run_until_done<F: Fn(&Sim)>(
        &mut self,
        map: &Map,
        callback: F,
        time_limit: Option<Tick>,
    ) {
        let mut benchmark = self.start_benchmark();
        loop {
            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.step(&map);
            })) {
                Ok(()) => {}
                Err(err) => {
                    println!("********************************************************************************");
                    println!("Sim broke:");
                    self.dump_before_abort();
                    panic::resume_unwind(err);
                }
            }

            if self.time.is_multiple_of(Tick::from_minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                println!("{0}, speed = {1:.2}x", self.summary(), speed);
            }
            callback(self);
            if Some(self.time) == time_limit {
                panic!("Time limit {} hit", self.time);
            }
            if self.is_done() {
                break;
            }
        }
    }

    pub fn run_until_expectations_met(
        &mut self,
        map: &Map,
        all_expectations: Vec<Event>,
        time_limit: Tick,
    ) {
        let mut benchmark = self.start_benchmark();
        let mut expectations = VecDeque::from(all_expectations);
        loop {
            if expectations.is_empty() {
                return;
            }
            for ev in self.step(&map).into_iter() {
                if ev == *expectations.front().unwrap() {
                    println!("At {}, met expectation {:?}", self.time, ev);
                    expectations.pop_front();
                    if expectations.is_empty() {
                        return;
                    }
                }
            }
            if self.time.is_multiple_of(Tick::from_minutes(1)) {
                let speed = self.measure_speed(&mut benchmark);
                println!("{0}, speed = {1:.2}x", self.summary(), speed);
            }
            if self.time == time_limit {
                panic!(
                    "Time limit {} hit, but some expectations never met: {:?}",
                    self.time, expectations
                );
            }
        }
    }
}

// Spawning helpers
impl Sim {
    pub fn small_spawn(&mut self, map: &Map) {
        let mut timer = Timer::new("small_spawn");

        // TODO This really ought to be part of the scenario
        for route in map.get_all_bus_routes() {
            self.seed_bus_route(route, map, &mut timer);
        }

        let mut s = Scenario {
            scenario_name: "small_spawn".to_string(),
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
        s.instantiate(self, map, &mut timer);
        timer.done();
    }

    pub fn seed_parked_cars(
        &mut self,
        owner_buildins: &Vec<BuildingID>,
        neighborhoods_roads: &BTreeSet<RoadID>,
        cars_per_building: &WeightedUsizeChoice,
        map: &Map,
        timer: &mut Timer,
    ) {
        self.spawner.seed_parked_cars(
            cars_per_building,
            owner_buildins,
            neighborhoods_roads,
            &mut self.parking_state,
            &mut self.rng,
            map,
            timer,
        );
    }

    // TODO This is for tests; rename or move it?
    pub fn seed_specific_parked_cars(
        &mut self,
        lane: LaneID,
        owner_building: BuildingID,
        spots: Vec<usize>,
    ) -> Vec<CarID> {
        self.spawner.seed_specific_parked_cars(
            lane,
            owner_building,
            spots,
            &mut self.parking_state,
            &mut self.rng,
        )
    }

    // TODO This is for tests; rename or move it?
    pub fn seed_trip_using_parked_car(
        &mut self,
        from_bldg: BuildingID,
        to_bldg: BuildingID,
        car: CarID,
        map: &Map,
    ) {
        self.spawner.start_trip_using_parked_car(
            Tick::zero(),
            map,
            self.parking_state.lookup_car(car).cloned().unwrap(),
            &self.parking_state,
            from_bldg,
            DrivingGoal::ParkNear(to_bldg),
            &mut self.trips_state,
        );
    }

    // TODO This is for tests; rename or move it?
    pub fn seed_trip_using_bus(
        &mut self,
        from_bldg: BuildingID,
        to_bldg: BuildingID,
        route: BusRouteID,
        stop1: BusStopID,
        stop2: BusStopID,
        map: &Map,
    ) -> PedestrianID {
        self.spawner.start_trip_using_bus(
            Tick::zero(),
            map,
            SidewalkSpot::building(from_bldg, map),
            SidewalkSpot::building(to_bldg, map),
            route,
            stop1,
            stop2,
            &mut self.trips_state,
        )
    }

    // TODO This is for tests and interactive UI; rename or move it?
    pub fn seed_trip_just_walking_to_bldg(
        &mut self,
        from_bldg: BuildingID,
        to_bldg: BuildingID,
        map: &Map,
    ) -> PedestrianID {
        self.spawner.start_trip_just_walking(
            self.time.next(),
            SidewalkSpot::building(from_bldg, map),
            SidewalkSpot::building(to_bldg, map),
            &mut self.trips_state,
        )
    }

    // TODO argh, duplication all over the everywhere
    pub fn seed_trip_just_walking_to_border(
        &mut self,
        from_bldg: BuildingID,
        to: IntersectionID,
        map: &Map,
    ) -> PedestrianID {
        self.spawner.start_trip_just_walking(
            self.time.next(),
            SidewalkSpot::building(from_bldg, map),
            SidewalkSpot::end_at_border(to, map).unwrap(),
            &mut self.trips_state,
        )
    }

    pub fn seed_trip_with_car_appearing_to_bldg(
        &mut self,
        from: Position,
        to_bldg: BuildingID,
        map: &Map,
    ) -> CarID {
        self.spawner.start_trip_with_car_appearing(
            self.time.next(),
            map,
            from,
            DrivingGoal::ParkNear(to_bldg),
            &mut self.trips_state,
            &mut self.rng,
        )
    }

    pub fn seed_trip_with_car_appearing_to_border(
        &mut self,
        from: Position,
        to: IntersectionID,
        map: &Map,
    ) -> CarID {
        self.spawner.start_trip_with_car_appearing(
            self.time.next(),
            map,
            from,
            DrivingGoal::Border(
                to,
                map.get_i(to).get_incoming_lanes(map, LaneType::Driving)[0],
            ),
            &mut self.trips_state,
            &mut self.rng,
        )
    }

    pub fn seed_bus_route(&mut self, route: &BusRoute, map: &Map, timer: &mut Timer) -> Vec<CarID> {
        // TODO throw away the events? :(
        let mut events: Vec<Event> = Vec::new();
        self.spawner.seed_bus_route(
            &mut events,
            route,
            &mut self.rng,
            map,
            &mut self.driving_state,
            &mut self.transit_state,
            &self.intersection_state,
            &mut self.trips_state,
            self.time,
            timer,
        )
    }
}
