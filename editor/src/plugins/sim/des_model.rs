use geom::{Acceleration, Distance, Duration, Speed};
use map_model::{LaneID, Map, Traversable};
use sim::{CarID, CarState, DrawCarInput, VehicleType};

pub fn get_state(time: Duration, map: &Map) -> Vec<DrawCarInput> {
    let lane = map.get_l(LaneID(1250));
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
    leader.accel_from_rest_to_speed_limit(0.5 * speed_limit);
    leader.freeflow(Duration::seconds(10.0));
    leader.deaccel_to_rest();

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
    follower.accel_from_rest_to_speed_limit(speed_limit);
    follower.freeflow(Duration::seconds(10.0));
    follower.deaccel_to_rest();

    // TODO Analytically find when and where collision will happen. Adjust the follower's intervals
    // somehow.

    let mut draw = Vec::new();
    if let Some(d) = leader.dist_at(time) {
        draw.push(draw_car(&leader, d, map));
    }
    if let Some(d) = follower.dist_at(time) {
        draw.push(draw_car(&follower, d, map));
    }
    draw
}

fn draw_car(car: &Car, front: Distance, map: &Map) -> DrawCarInput {
    let lane = map.get_l(LaneID(1250));

    DrawCarInput {
        id: car.id,
        waiting_for_turn: None,
        stopping_trace: None,
        state: car.state,
        vehicle_type: VehicleType::Car,
        on: Traversable::Lane(lane.id),
        body: lane
            .lane_center_pts
            .slice(front - car.car_length, front)
            .unwrap()
            .0,
    }
}

struct Interval {
    start_dist: Distance,
    end_dist: Distance,
    start_time: Duration,
    end_time: Duration,
    start_speed: Speed,
    end_speed: Speed,
    // Extra info: CarID, LaneID
}

impl Interval {
    fn new(
        start_dist: Distance,
        end_dist: Distance,
        start_time: Duration,
        end_time: Duration,
        start_speed: Speed,
        end_speed: Speed,
    ) -> Interval {
        assert!(start_dist >= Distance::ZERO);
        assert!(start_time >= Duration::ZERO);
        assert!(start_dist < end_dist);
        assert!(start_time < end_time);
        assert!(start_speed >= Speed::ZERO);
        assert!(end_speed >= Speed::ZERO);
        Interval {
            start_dist,
            end_dist,
            start_time,
            end_time,
            start_speed,
            end_speed,
        }
    }

    fn dist(&self, t: Duration) -> Distance {
        // Linearly interpolate
        self.start_dist + self.percent(t) * (self.end_dist - self.start_dist)
    }

    fn speed(&self, t: Duration) -> Speed {
        // Linearly interpolate
        self.start_speed + self.percent(t) * (self.end_speed - self.start_speed)
    }

    fn covers(&self, t: Duration) -> bool {
        t >= self.start_time && t <= self.end_time
    }

    fn percent(&self, t: Duration) -> f64 {
        assert!(self.covers(t));
        (t - self.start_time) / (self.end_time - self.start_time)
    }

    // TODO intersection operation figures out dist and speed by interpolating, of course.
}

// TODO use lane length and a car's properties to figure out reasonable intervals for short/long
// lanes
// TODO debug draw an interval

struct Car {
    id: CarID,
    // Hack used for different colors
    state: CarState,
    car_length: Distance,
    max_accel: Acceleration,
    max_deaccel: Acceleration,

    start_dist: Distance,
    start_time: Duration,

    intervals: Vec<Interval>,
}

impl Car {
    // TODO If we had a constructor, make sure start_dist >= car_length.

    // None if they're not on the lane by then
    fn dist_at(&self, t: Duration) -> Option<Distance> {
        // TODO Binary search
        for (idx, i) in self.intervals.iter().enumerate() {
            if i.covers(t) {
                // TODO Show this in the modal menu.
                println!(
                    "{} is doing interval {}/{}. Speed {}",
                    self.id,
                    idx + 1,
                    self.intervals.len(),
                    i.speed(t)
                );
                return Some(i.dist(t));
            }
        }
        None
    }

    fn last_state(&self) -> (Distance, Speed, Duration) {
        if let Some(i) = self.intervals.last() {
            (i.end_dist, i.end_speed, i.end_time)
        } else {
            (self.start_dist, Speed::ZERO, self.start_time)
        }
    }

    fn next_state(&mut self, dist_covered: Distance, final_speed: Speed, time_needed: Duration) {
        let (dist1, speed1, time1) = self.last_state();
        self.intervals.push(Interval::new(
            dist1,
            dist1 + dist_covered,
            time1,
            time1 + time_needed,
            speed1,
            final_speed,
        ));
    }

    fn accel_from_rest_to_speed_limit(&mut self, speed: Speed) {
        assert_eq!(self.last_state().1, Speed::ZERO);

        // v_f = v_0 + a(t)
        let time_needed = speed / self.max_accel;

        // d = (v_0)(t) + (1/2)(a)(t^2)
        // TODO Woops, don't have Duration^2
        let dist_covered = Distance::meters(
            0.5 * self.max_accel.inner_meters_per_second_squared()
                * time_needed.inner_seconds().powi(2),
        );

        self.next_state(dist_covered, speed, time_needed);
    }

    fn freeflow(&mut self, time: Duration) {
        let speed = self.last_state().1;
        // Should explicitly wait for some time
        assert_ne!(speed, Speed::ZERO);

        self.next_state(speed * time, speed, time);
    }

    fn deaccel_to_rest(&mut self) {
        let speed = self.last_state().1;
        assert_ne!(speed, Speed::ZERO);

        // v_f = v_0 + a(t)
        let time_needed = -speed / self.max_deaccel;

        // d = (v_0)(t) + (1/2)(a)(t^2)
        let dist_covered = speed * time_needed
            + Distance::meters(
                0.5 * self.max_deaccel.inner_meters_per_second_squared()
                    * time_needed.inner_seconds().powi(2),
            );

        self.next_state(dist_covered, Speed::ZERO, time_needed);
    }
}
