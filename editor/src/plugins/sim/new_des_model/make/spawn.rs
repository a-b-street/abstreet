use crate::plugins::sim::new_des_model::{
    Command, CreateCar, CreatePedestrian, DrivingGoal, ParkingSimState, ParkingSpot, Router,
    Scheduler, SidewalkSpot, TripLeg, TripManager, VehicleSpec,
};
use abstutil::Timer;
use geom::Duration;
use map_model::{BusRouteID, BusStopID, Map, Path, PathRequest, Pathfinder, Position};
use serde_derive::{Deserialize, Serialize};
use sim::{CarID, PedestrianID, VehicleType};
use std::collections::BTreeSet;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum TripSpec {
    // Can be used to spawn from a border or anywhere for interactive debugging.
    CarAppearing(Position, VehicleSpec, DrivingGoal),
    UsingParkedCar(SidewalkSpot, ParkingSpot, DrivingGoal),
    JustWalking(SidewalkSpot, SidewalkSpot),
    UsingBike(SidewalkSpot, VehicleSpec, DrivingGoal),
    UsingTransit(SidewalkSpot, BusRouteID, BusStopID, BusStopID, SidewalkSpot),
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct TripSpawner {
    // TODO tmp pub
    pub(crate) car_id_counter: usize,
    ped_id_counter: usize,
    parked_cars_claimed: BTreeSet<CarID>,
    trips: Vec<(Duration, TripSpec)>,
}

impl TripSpawner {
    pub fn new() -> TripSpawner {
        TripSpawner {
            car_id_counter: 0,
            ped_id_counter: 0,
            parked_cars_claimed: BTreeSet::new(),
            trips: Vec::new(),
        }
    }

    pub fn schedule_trip(
        &mut self,
        start_time: Duration,
        spec: TripSpec,
        map: &Map,
        parking: &ParkingSimState,
    ) {
        // TODO We'll want to repeat this validation when we spawn stuff later for a second leg...
        match &spec {
            TripSpec::CarAppearing(start_pos, vehicle_spec, goal) => {
                if start_pos.dist_along() < vehicle_spec.length {
                    panic!(
                        "Can't spawn a car at {}; too close to the start",
                        start_pos.dist_along()
                    );
                }
                if start_pos.dist_along() >= map.get_l(start_pos.lane()).length() {
                    panic!(
                        "Can't spawn a car at {}; {} isn't that long",
                        start_pos.dist_along(),
                        start_pos.lane()
                    );
                }
                match goal {
                    DrivingGoal::Border(_, end_lane) => {
                        if start_pos.lane() == *end_lane
                            && start_pos.dist_along() == map.get_l(*end_lane).length()
                        {
                            panic!("Can't start a car at the edge of a border already");
                        }
                    }
                    DrivingGoal::ParkNear(_) => {}
                }
            }
            TripSpec::UsingParkedCar(_, spot, _) => {
                let car_id = parking.get_car_at_spot(*spot).unwrap().vehicle.id;
                if self.parked_cars_claimed.contains(&car_id) {
                    panic!(
                        "A TripSpec wants to use {}, which is already claimed",
                        car_id
                    );
                }
                self.parked_cars_claimed.insert(car_id);
            }
            TripSpec::JustWalking(_, _) => {}
            TripSpec::UsingBike(start, _, _) => {
                if SidewalkSpot::bike_rack(start.sidewalk_pos.lane(), map).is_none() {
                    panic!(
                        "Can't start biking from {}; no biking or driving lane nearby?",
                        start.sidewalk_pos.lane()
                    );
                }
            }
            TripSpec::UsingTransit(_, _, _, _, _) => {}
        };

        self.trips.push((start_time, spec));
    }

    pub fn spawn_all(
        &mut self,
        map: &Map,
        parking: &ParkingSimState,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        timer: &mut Timer,
    ) {
        let paths = calculate_paths(
            map,
            self.trips
                .iter()
                .map(|(_, spec)| spec.get_pathfinding_request(map, parking))
                .collect(),
            timer,
        );
        for ((start_time, spec), maybe_path) in self.trips.drain(..).zip(paths) {
            if maybe_path.is_none() {
                timer.warn(format!("{:?} couldn't find the first path", spec));
                continue;
            }
            let path = maybe_path.unwrap();
            match spec {
                TripSpec::CarAppearing(start_pos, vehicle_spec, goal) => {
                    let car_id = CarID::tmp_new(self.car_id_counter, VehicleType::Car);
                    self.car_id_counter += 1;
                    let ped_id = PedestrianID::tmp_new(self.ped_id_counter);
                    self.ped_id_counter += 1;

                    let mut legs = vec![TripLeg::Drive(car_id, goal.clone())];
                    let router = match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                            Router::park_near(path, b)
                        }
                        DrivingGoal::Border(_, last_lane) => {
                            Router::stop_suddenly(path, map.get_l(last_lane).length())
                        }
                    };
                    let trip = trips.new_trip(start_time, Some(ped_id), legs);

                    scheduler.enqueue_command(Command::SpawnCar(
                        start_time,
                        CreateCar::for_appearing(
                            vehicle_spec.make(car_id),
                            start_pos,
                            router,
                            trip,
                        ),
                    ));
                }
                TripSpec::UsingParkedCar(start, spot, goal) => {
                    let ped_id = PedestrianID::tmp_new(self.ped_id_counter);
                    self.ped_id_counter += 1;
                    let car_id = parking.get_car_at_spot(spot).unwrap().vehicle.id;
                    //assert_eq!(parked.owner, Some(start_bldg));

                    let parking_spot = SidewalkSpot::parking_spot(spot, map, parking);

                    let mut legs = vec![
                        TripLeg::Walk(parking_spot.clone()),
                        TripLeg::Drive(car_id, goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                        }
                        DrivingGoal::Border(_, _) => {}
                    }
                    let trip = trips.new_trip(start_time, Some(ped_id), legs);

                    scheduler.enqueue_command(Command::SpawnPed(
                        start_time,
                        CreatePedestrian {
                            id: ped_id,
                            start,
                            goal: parking_spot,
                            path,
                            trip,
                        },
                    ));
                }
                TripSpec::JustWalking(start, goal) => {
                    let ped_id = PedestrianID::tmp_new(self.ped_id_counter);
                    self.ped_id_counter += 1;

                    let trip =
                        trips.new_trip(start_time, Some(ped_id), vec![TripLeg::Walk(goal.clone())]);

                    scheduler.enqueue_command(Command::SpawnPed(
                        start_time,
                        CreatePedestrian {
                            id: ped_id,
                            start,
                            goal,
                            path,
                            trip,
                        },
                    ));
                }
                TripSpec::UsingBike(start, vehicle, goal) => {
                    let ped_id = PedestrianID::tmp_new(self.ped_id_counter);
                    self.ped_id_counter += 1;
                    let bike_id = CarID::tmp_new(self.car_id_counter, VehicleType::Bike);
                    self.car_id_counter += 1;

                    let walk_to = SidewalkSpot::bike_rack(start.sidewalk_pos.lane(), map).unwrap();
                    let mut legs = vec![
                        TripLeg::Walk(walk_to.clone()),
                        TripLeg::Bike(vehicle.make(bike_id), goal.clone()),
                    ];
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            legs.push(TripLeg::Walk(SidewalkSpot::building(b, map)));
                        }
                        DrivingGoal::Border(_, _) => {}
                    }
                    let trip = trips.new_trip(start_time, Some(ped_id), legs);

                    scheduler.enqueue_command(Command::SpawnPed(
                        start_time,
                        CreatePedestrian {
                            id: ped_id,
                            start,
                            goal: walk_to,
                            path,
                            trip,
                        },
                    ));
                }
                TripSpec::UsingTransit(_, _, _, _, _) => {
                    panic!("implement");
                }
            }
        }
    }

    pub fn is_done(&self) -> bool {
        self.trips.is_empty()
    }
}

impl TripSpec {
    fn get_pathfinding_request(&self, map: &Map, parking: &ParkingSimState) -> PathRequest {
        match self {
            TripSpec::CarAppearing(start, vehicle_spec, goal) => PathRequest {
                start: *start,
                end: goal.goal_pos(map),
                can_use_bus_lanes: vehicle_spec.vehicle_type == VehicleType::Bus,
                can_use_bike_lanes: vehicle_spec.vehicle_type == VehicleType::Bike,
            },
            TripSpec::UsingParkedCar(start, spot, _) => PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::parking_spot(*spot, map, parking).sidewalk_pos,
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            TripSpec::JustWalking(start, goal) => PathRequest {
                start: start.sidewalk_pos,
                end: goal.sidewalk_pos,
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            TripSpec::UsingBike(start, _, _) => PathRequest {
                start: start.sidewalk_pos,
                end: SidewalkSpot::bike_rack(start.sidewalk_pos.lane(), map)
                    .unwrap()
                    .sidewalk_pos,
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            TripSpec::UsingTransit(_, _, _, _, _) => {
                panic!("implement");
            }
        }
    }
}

fn calculate_paths(map: &Map, requests: Vec<PathRequest>, timer: &mut Timer) -> Vec<Option<Path>> {
    use rayon::prelude::*;

    timer.start(&format!("calculate {} paths", requests.len()));
    let paths: Vec<Option<Path>> = requests
        .into_par_iter()
        .map(|req| Pathfinder::shortest_distance(map, req))
        .collect();
    timer.stop(&format!("calculate {} paths", paths.len()));
    paths
}
