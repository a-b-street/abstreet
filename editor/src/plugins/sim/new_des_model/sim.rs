use crate::plugins::sim::new_des_model::{
    DrivingSimState, ParkedCar, ParkingSimState, Router, SidewalkSpot, Vehicle, WalkingSimState,
};
use ezgui::GfxCtx;
use geom::{Distance, Duration};
use map_model::{Map, Path, Traversable};
use sim::{DrawCarInput, DrawPedestrianInput, PedestrianID};

pub struct Sim {
    driving: DrivingSimState,
    // TODO pub just for lazy spawning
    pub parking: ParkingSimState,
    walking: WalkingSimState,
}

impl Sim {
    pub fn new(map: &Map) -> Sim {
        Sim {
            driving: DrivingSimState::new(map),
            parking: ParkingSimState::new(map),
            walking: WalkingSimState::new(),
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

    pub fn spawn_car(
        &mut self,
        vehicle: Vehicle,
        router: Router,
        start_time: Duration,
        start_dist: Distance,
        maybe_parked_car: Option<ParkedCar>,
        map: &Map,
    ) {
        self.driving.spawn_car(
            vehicle,
            router,
            start_time,
            start_dist,
            maybe_parked_car,
            map,
            &self.parking,
        );
    }

    pub fn spawn_ped(
        &mut self,
        id: PedestrianID,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        path: Path,
        map: &Map,
    ) {
        let start_time = Duration::ZERO;
        self.walking
            .spawn_ped(id, start_time, start, goal, path, map);
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map) {
        self.driving.step_if_needed(time, map, &mut self.parking);
    }
}
