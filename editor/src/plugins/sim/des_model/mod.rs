mod car;
mod interval;

use crate::objects::DrawCtx;
use crate::plugins::sim::des_model::car::Car;
use ezgui::{GfxCtx, Text};
use geom::{Acceleration, Distance, Duration};
use map_model::{LaneID, Map};
use sim::{CarID, CarState, DrawCarInput, VehicleType};

pub struct World {
    lane: LaneID,
    leader: Car,
    follower: Car,
}

impl World {
    pub fn new(l: LaneID, map: &Map) -> World {
        let lane = map.get_l(l);
        let speed_limit = map.get_parent(lane.id).get_speed_limit();

        let mut leader = Car {
            id: CarID::tmp_new(0, VehicleType::Car),
            state: CarState::Moving,
            car_length: Distance::meters(5.0),
            max_accel: Acceleration::meters_per_second_squared(2.5),
            max_deaccel: Acceleration::meters_per_second_squared(-3.0),
            intervals: Vec::new(),
            start_dist: Distance::meters(5.0),
            start_time: Duration::ZERO,
        };
        leader.stop_at_end_of_lane(lane, 0.5 * speed_limit);
        leader.wait(Duration::seconds(5.0));

        let mut follower = Car {
            id: CarID::tmp_new(1, VehicleType::Car),
            state: CarState::Stuck,
            car_length: Distance::meters(5.0),
            max_accel: Acceleration::meters_per_second_squared(4.5),
            max_deaccel: Acceleration::meters_per_second_squared(-2.0),
            intervals: Vec::new(),
            start_dist: Distance::meters(5.0),
            start_time: Duration::seconds(4.0),
        };
        follower.stop_at_end_of_lane(lane, speed_limit);
        follower.wait(Duration::seconds(5.0));

        println!("Leader:\n");
        leader.dump_intervals();
        println!("\nOriginal follower:\n");
        follower.dump_intervals();
        println!();

        follower.maybe_follow(&mut leader);
        println!("\nAdjusted follower:\n");
        follower.dump_intervals();
        println!();

        leader.validate(lane);
        follower.validate(lane);
        World {
            leader,
            follower,
            lane: l,
        }
    }

    pub fn get_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut draw = Vec::new();
        for car in &[&self.leader, &self.follower] {
            if let Some((d, _)) = car.dist_at(time) {
                draw.push(car.get_draw_car(d, map.get_l(self.lane)));
            }
        }
        draw
    }

    pub fn draw_tooltips(&self, g: &mut GfxCtx, ctx: &DrawCtx, time: Duration) {
        let lane = ctx.map.get_l(self.lane);

        for car in &[&self.leader, &self.follower] {
            if let Some((d, idx)) = car.dist_at(time) {
                g.draw_text_at(
                    Text::from_line(format!(
                        "Interval {}/{}, speed {}",
                        idx + 1,
                        car.intervals.len(),
                        car.intervals[idx].speed(time)
                    )),
                    lane.lane_center_pts.dist_along(d - 0.5 * car.car_length).0,
                );
            }
        }
    }
}
