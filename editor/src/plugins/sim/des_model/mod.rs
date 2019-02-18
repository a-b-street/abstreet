mod car;
mod interval;

use crate::objects::DrawCtx;
use crate::plugins::sim::des_model::car::Car;
use ezgui::{GfxCtx, Text};
use geom::{Acceleration, Distance, Duration, Speed, EPSILON_DIST};
use map_model::{LaneID, Map};
use sim::{CarID, CarState, DrawCarInput, VehicleType};

pub struct World {
    pub lane: LaneID,
    cars: Vec<Car>,
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
            // TODO Enter with some speed
            //start_speed: 0.2 * speed_limit,
            start_speed: Speed::ZERO,
        };
        leader.start_then_stop(lane.length(), 0.5 * speed_limit);
        leader.wait(Duration::seconds(5.0));

        let mut cars = vec![leader];
        let num_followers = (lane.length() / Distance::meters(10.0)).floor() as usize;
        for i in 0..num_followers {
            let mut follower = Car {
                id: CarID::tmp_new(cars.len(), VehicleType::Car),
                state: CarState::Stuck,
                car_length: Distance::meters(5.0),
                max_accel: Acceleration::meters_per_second_squared(4.5),
                max_deaccel: Acceleration::meters_per_second_squared(-2.0),
                intervals: Vec::new(),
                start_dist: Distance::meters(5.0),
                start_time: ((i + 1) as f64) * Duration::seconds(4.0),
                start_speed: Speed::ZERO,
            };
            follower.start_then_stop(lane.length(), speed_limit);
            follower.maybe_follow(cars.last().unwrap());
            follower.start_then_stop(lane.length(), speed_limit);
            follower.wait(Duration::seconds(5.0));
            cars.push(follower);
        }

        for c in &cars {
            /*println!("{}:\n", c.id);
            c.dump_intervals();
            println!();*/
            c.validate(lane);
            //println!();
        }

        World { lane: l, cars }
    }

    pub fn get_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut draw = Vec::new();
        for follower in &self.cars {
            if let Some((d, _)) = follower.dist_at(time) {
                draw.push(follower.get_draw_car(d, map.get_l(self.lane)));
            }
        }
        draw
    }

    pub fn draw_tooltips(&self, g: &mut GfxCtx, ctx: &DrawCtx, time: Duration) {
        let lane = ctx.map.get_l(self.lane);

        for car in &self.cars {
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

    pub fn dump_debug(&self, time: Duration) {
        for car in &self.cars {
            if let Some((d, idx)) = car.dist_at(time) {
                println!(
                    "- {} at {}, speed {}. interval {}/{}",
                    car.id,
                    d,
                    car.intervals[idx].speed(time),
                    idx + 1,
                    car.intervals.len()
                );
            }
        }
    }

    pub fn sample_for_proximity(&self) {
        let mut time = Duration::ZERO;
        loop {
            let mut max_dist: Option<Distance> = None;
            let mut active_cars = 0;
            for (car_idx, follower) in self.cars.iter().enumerate() {
                if let Some((d, follower_idx)) = follower.dist_at(time) {
                    active_cars += 1;
                    if let Some(max) = max_dist {
                        if d > max + EPSILON_DIST {
                            let leader = &self.cars[car_idx - 1];
                            let leader_idx = leader.dist_at(time).unwrap().1;
                            println!("{} is too close to {} at {}", follower.id, leader.id, time);
                            println!("leader doing: {}", leader.intervals[leader_idx]);
                            println!("follower doing: {}", follower.intervals[follower_idx]);
                            panic!(
                                "to repro:\n\n{}.intersection(&{})\n\n",
                                follower.intervals[follower_idx].repr(),
                                leader.intervals[leader_idx].repr()
                            );
                        }
                    }
                    max_dist = Some(d - follower.car_length - car::FOLLOWING_DISTANCE);
                }
            }
            // All the cars are done
            if max_dist.is_none() {
                return;
            }
            time += Duration::seconds(0.1);
            if time.is_multiple_of(Duration::seconds(10.0)) {
                println!(
                    "Checking {}. {} cars at {}...",
                    self.lane, active_cars, time
                );
            }
        }
    }
}
