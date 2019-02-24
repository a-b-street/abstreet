use crate::plugins::sim::new_des_model::{
    Command, CreateCar, CreatePedestrian, DrivingGoal, ParkingSimState, ParkingSpot, Router,
    Scheduler, SidewalkSpot, TripLeg, TripManager, VehicleSpec,
};
use abstutil::Timer;
use geom::Duration;
use map_model::{Map, Path, PathRequest, Pathfinder, Position};
use sim::{CarID, PedestrianID, VehicleType};
use std::collections::BTreeSet;

#[derive(Debug)]
pub enum TripSpec {
    // Can be used to spawn from a border or anywhere for interactive debugging.
    CarAppearing(Position, VehicleSpec, DrivingGoal),
    // TODO The only starts that really make sense are building or border...
    UsingParkedCar(SidewalkSpot, ParkingSpot, DrivingGoal),
    // TODO The only starts that really make sense are building or border...
    //UsingBike(SidewalkSpot, VehicleSpec, DrivingGoal),
    JustWalking(SidewalkSpot, SidewalkSpot),
    // TODO Transit
}

pub struct TripSpawner {
    car_id_counter: usize,
    ped_id_counter: usize,
    parked_cars_claimed: BTreeSet<CarID>,
}

impl TripSpawner {
    pub fn new() -> TripSpawner {
        TripSpawner {
            car_id_counter: 0,
            ped_id_counter: 0,
            parked_cars_claimed: BTreeSet::new(),
        }
    }

    pub fn schedule_trip(
        &mut self,
        start_time: Duration,
        spec: TripSpec,
        path: Path,
        map: &Map,
        parking: &ParkingSimState,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
    ) {
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
                        Router::park_near(path.convert_to_traversable_list(), b)
                    }
                    DrivingGoal::Border(_, last_lane) => Router::stop_suddenly(
                        path.convert_to_traversable_list(),
                        map.get_l(last_lane).length(),
                        map,
                    ),
                };
                let trip = trips.new_trip(start_time, Some(ped_id), legs);

                scheduler.enqueue_command(Command::SpawnCar(
                    start_time,
                    CreateCar {
                        vehicle: vehicle_spec.make(car_id),
                        router,
                        start_dist: start_pos.dist_along(),
                        maybe_parked_car: None,
                        trip,
                    },
                ));
            }
            TripSpec::UsingParkedCar(start, spot, goal) => {
                let ped_id = PedestrianID::tmp_new(self.ped_id_counter);
                self.ped_id_counter += 1;
                let car_id = parking.get_car_at_spot(spot);

                if self.parked_cars_claimed.contains(&car_id) {
                    panic!(
                        "A TripSpec wants to use {}, which is already claimed",
                        car_id
                    );
                }
                self.parked_cars_claimed.insert(car_id);

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
        }
    }
}

impl TripSpec {
    fn get_pathfinding_request(&self, map: &Map, parking: &ParkingSimState) -> PathRequest {
        match self {
            TripSpec::CarAppearing(start, vehicle_spec, goal) => {
                let goal_lane = match goal {
                    DrivingGoal::ParkNear(b) => map.find_driving_lane_near_building(*b),
                    DrivingGoal::Border(_, l) => *l,
                };
                PathRequest {
                    start: *start,
                    end: Position::new(goal_lane, map.get_l(goal_lane).length()),
                    can_use_bus_lanes: vehicle_spec.vehicle_type == VehicleType::Bus,
                    can_use_bike_lanes: vehicle_spec.vehicle_type == VehicleType::Bike,
                }
            }
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
