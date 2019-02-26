use crate::plugins::sim::new_des_model::{
    DrivingSimState, IntersectionSimState, ParkedCar, ParkingSimState, ParkingSpot, Scheduler,
    TripManager, TripSpawner, TripSpec, VehicleSpec, WalkingSimState,
};
use abstutil::Timer;
use ezgui::GfxCtx;
use geom::Duration;
use map_model::{BuildingID, LaneID, Map, Traversable};
use serde_derive::{Deserialize, Serialize};
use sim::{CarID, DrawCarInput, DrawPedestrianInput, VehicleType};

#[derive(Serialize, Deserialize)]
pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    trips: TripManager,
    scheduler: Scheduler,
    spawner: TripSpawner,
    time: Duration,

    // TODO Reconsider these
    pub(crate) map_name: String,
    pub(crate) edits_name: String,
}

impl Sim {
    pub fn new(map: &Map, run_name: String, savestate_every: Option<Duration>) -> Sim {
        Sim {
            driving: DrivingSimState::new(map),
            parking: ParkingSimState::new(map),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(map),
            trips: TripManager::new(),
            scheduler: Scheduler::new(),
            spawner: TripSpawner::new(),
            time: Duration::ZERO,

            map_name: map.get_name().to_string(),
            edits_name: map.get_edits().edits_name.to_string(),
        }
    }

    pub fn schedule_trip(&mut self, start_time: Duration, spec: TripSpec, map: &Map) {
        self.spawner
            .schedule_trip(start_time, spec, map, &self.parking);
    }

    pub fn spawn_all_trips(&mut self, map: &Map, timer: &mut Timer) {
        self.spawner.spawn_all(
            map,
            &self.parking,
            &mut self.trips,
            &mut self.scheduler,
            timer,
        );
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_spots(l)
    }

    pub fn seed_parked_car(
        &mut self,
        vehicle: VehicleSpec,
        spot: ParkingSpot,
        owner: Option<BuildingID>,
    ) {
        self.parking.reserve_spot(spot);
        self.parking.add_parked_car(ParkedCar::new(
            vehicle.make(CarID::tmp_new(
                self.spawner.car_id_counter,
                VehicleType::Car,
            )),
            spot,
            owner,
        ));
        self.spawner.car_id_counter += 1;
    }

    pub fn get_parked_cars_by_owner(&self, bldg: BuildingID) -> Vec<&ParkedCar> {
        self.parking.get_parked_cars_by_owner(bldg)
    }
}

impl Sim {
    pub fn draw_unzoomed(&self, g: &mut GfxCtx, map: &Map) {
        self.driving.draw_unzoomed(self.time, g, map);
    }

    pub fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        let mut result = self.driving.get_all_draw_cars(self.time, map);
        result.extend(self.parking.get_all_draw_cars(map));
        result
    }

    pub fn get_draw_cars_on(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
        if let Traversable::Lane(l) = on {
            if map.get_l(l).is_parking() {
                return self.parking.get_draw_cars(l, map);
            }
        }
        self.driving.get_draw_cars_on(self.time, on, map)
    }

    pub fn get_all_draw_peds(&self, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_all_draw_peds(self.time, map)
    }

    pub fn get_draw_peds_on(&self, on: Traversable, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_draw_peds(self.time, on, map)
    }
}

impl Sim {
    pub fn step_if_needed(&mut self, map: &Map) {
        self.time += Duration::seconds(0.1);

        self.driving.step_if_needed(
            self.time,
            map,
            &mut self.parking,
            &mut self.intersections,
            &mut self.trips,
            &mut self.scheduler,
        );
        self.walking.step_if_needed(
            self.time,
            map,
            &mut self.intersections,
            &self.parking,
            &mut self.scheduler,
            &mut self.trips,
        );

        // Spawn stuff at the end, so we can see the correct state of everything else at this time.
        self.scheduler.step_if_needed(
            self.time,
            map,
            &mut self.parking,
            &mut self.walking,
            &mut self.driving,
            &self.intersections,
            &mut self.trips,
        );
    }
}

impl Sim {
    pub fn time(&self) -> Duration {
        self.time
    }
}
