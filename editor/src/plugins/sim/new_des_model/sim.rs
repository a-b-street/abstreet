use crate::plugins::sim::new_des_model::{DrivingSimState, ParkingSimState, Vehicle};
use ezgui::GfxCtx;
use geom::{Distance, Duration};
use map_model::{Map, Traversable};
use sim::DrawCarInput;

pub struct Sim {
    driving: DrivingSimState,
    // TODO pub just for lazy spawning
    pub parking: ParkingSimState,
}

impl Sim {
    pub fn new(map: &Map) -> Sim {
        Sim {
            driving: DrivingSimState::new(map),
            parking: ParkingSimState::new(map),
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

    pub fn spawn_car(
        &mut self,
        vehicle: Vehicle,
        path: Vec<Traversable>,
        start_time: Duration,
        start_dist: Distance,
        end_dist: Distance,
        map: &Map,
    ) {
        self.driving
            .spawn_car(vehicle, path, start_time, start_dist, end_dist, map);
    }

    pub fn step_if_needed(&mut self, time: Duration, map: &Map) {
        self.driving.step_if_needed(time, map);
    }
}
