use crate::plugins::sim::new_des_model::{
    DrivingSimState, IntersectionSimState, ParkedCar, ParkingSimState, ParkingSpot, Scheduler,
    TripManager, TripSpawner, TripSpec, Vehicle, WalkingSimState,
};
use abstutil::Timer;
use ezgui::GfxCtx;
use geom::Duration;
use map_model::{LaneID, Map, Position, Traversable};
use sim::{CarID, DrawCarInput, DrawPedestrianInput};

pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    trips: TripManager,
    scheduler: Scheduler,
    spawner: TripSpawner,
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
            spawner: TripSpawner::new(),
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

    pub fn schedule_trip(&mut self, start_time: Duration, spec: TripSpec) {
        self.spawner.schedule_trip(start_time, spec);
    }

    pub fn spawn_all_trips(&mut self, map: &Map) {
        let mut timer = Timer::new("spawn all trips");
        self.spawner.spawn_all(
            map,
            &self.parking,
            &mut self.trips,
            &mut self.scheduler,
            &mut timer,
        );
        timer.done();
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

    pub fn seed_parked_car(&mut self, mut parked_car: ParkedCar) {
        // TODO tmp hack.
        parked_car.vehicle.id =
            CarID::tmp_new(self.spawner.car_id_counter, parked_car.vehicle.vehicle_type);
        self.spawner.car_id_counter += 1;

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
