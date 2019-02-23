use crate::plugins::sim::new_des_model::{
    Command, CreateCar, CreatePedestrian, DrivingSimState, IntersectionSimState, ParkedCar,
    ParkingSimState, ParkingSpot, Router, Scheduler, SidewalkSpot, TripManager, Vehicle,
    WalkingSimState,
};
use ezgui::GfxCtx;
use geom::{Distance, Duration};
use map_model::{LaneID, Map, Path, Position, Traversable};
use sim::{DrawCarInput, DrawPedestrianInput, PedestrianID, TripID};

pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    trips: TripManager,
    scheduler: Scheduler,
}

impl Sim {
    pub fn new(map: &Map) -> Sim {
        Sim {
            driving: DrivingSimState::new(map),
            parking: ParkingSimState::new(map),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(map),
            trips: TripManager::new(),
            scheduler: Scheduler::new(),
        }
    }

    pub fn draw_unzoomed(&self, time: Duration, g: &mut GfxCtx, map: &Map) {
        self.driving.draw_unzoomed(time, g, map);
    }

    pub fn get_all_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut result = self.driving.get_all_draw_cars(time, map);
        result.extend(self.parking.get_all_draw_cars(map));
        result
    }

    pub fn get_draw_cars_on(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawCarInput> {
        if let Traversable::Lane(l) = on {
            if map.get_l(l).is_parking() {
                return self.parking.get_draw_cars(l, map);
            }
        }
        self.driving.get_draw_cars_on(time, on, map)
    }

    pub fn get_all_draw_peds(&self, time: Duration, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_all_draw_peds(time, map)
    }

    pub fn get_draw_peds_on(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawPedestrianInput> {
        self.walking.get_draw_peds(time, on, map)
    }

    // TODO Many of these should go away
    pub fn spawn_car(
        &mut self,
        vehicle: Vehicle,
        router: Router,
        start_time: Duration,
        start_dist: Distance,
        maybe_parked_car: Option<ParkedCar>,
    ) {
        self.scheduler.enqueue_command(Command::SpawnCar(
            start_time,
            CreateCar {
                vehicle,
                router,
                start_dist,
                maybe_parked_car,
                trip: TripID(0),
            },
        ));
    }

    pub fn spawn_ped(
        &mut self,
        id: PedestrianID,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        path: Path,
    ) {
        self.scheduler.enqueue_command(Command::SpawnPed(
            Duration::ZERO,
            CreatePedestrian {
                id,
                start,
                goal,
                path,
                trip: TripID(0),
            },
        ));
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_spots(l)
    }

    // TODO Ew...
    pub fn spot_to_driving_pos(
        &self,
        spot: ParkingSpot,
        vehicle: &Vehicle,
        driving_lane: LaneID,
        map: &Map,
    ) -> Position {
        self.parking
            .spot_to_driving_pos(spot, vehicle, driving_lane, map)
    }

    pub fn seed_parked_car(&mut self, parked_car: ParkedCar) {
        self.parking.reserve_spot(parked_car.spot);
        self.parking.add_parked_car(parked_car);
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map) {
        self.driving
            .step_if_needed(time, map, &mut self.parking, &mut self.intersections);
        self.walking
            .step_if_needed(time, map, &mut self.intersections);

        // Spawn stuff at the end, so we can see the correct state of everything else at this time.
        self.scheduler.step_if_needed(
            time,
            map,
            &mut self.parking,
            &mut self.walking,
            &mut self.driving,
            &self.intersections,
            &mut self.trips,
        );
    }
}
