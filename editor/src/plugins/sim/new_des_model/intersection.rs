pub use crate::plugins::sim::new_des_model::{Car, CarState, TimeInterval};
use geom::Duration;
use map_model::{IntersectionID, Map};
use sim::DrawCarInput;

pub struct IntersectionController {
    pub id: IntersectionID,
    pub accepted: Option<Car>,
}

impl IntersectionController {
    pub fn get_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        if let Some(ref car) = self.accepted {
            let t = map.get_t(car.path[0].as_turn());
            let percent = match car.state {
                CarState::CrossingTurn(ref int) => int.percent(time),
                _ => unreachable!(),
            };
            if let Some(d) = car.get_draw_car(percent * t.geom.length(), map) {
                return vec![d];
            }
        }
        Vec::new()
    }
}
