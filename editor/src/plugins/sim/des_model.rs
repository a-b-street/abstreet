use geom::{Acceleration, Distance, Duration, Speed};
use map_model::{LaneID, Map, Traversable};
use sim::{CarID, CarState, DrawCarInput, VehicleType};

pub struct World {
    leader: Car,
    follower: Car,
}

impl World {
    pub fn new(map: &Map) -> World {
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

        println!("Leader:\n");
        for i in &leader.intervals {
            println!(
                "- {}->{} during {}->{} ({}->{})",
                i.start_dist, i.end_dist, i.start_time, i.end_time, i.start_speed, i.end_speed
            );
        }
        println!("\nOriginal follower:\n");
        for i in &follower.intervals {
            println!(
                "- {}->{} during {}->{} ({}->{})",
                i.start_dist, i.end_dist, i.start_time, i.end_time, i.start_speed, i.end_speed
            );
        }
        println!("");

        follower.maybe_follow(&mut leader);
        println!("\nAdjusted follower:\n");
        for i in &follower.intervals {
            println!(
                "- {}->{} during {}->{} ({}->{})",
                i.start_dist, i.end_dist, i.start_time, i.end_time, i.start_speed, i.end_speed
            );
        }
        println!("");

        leader.validate();
        follower.validate();
        World { leader, follower }
    }

    pub fn get_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut draw = Vec::new();
        if let Some(d) = self.leader.dist_at(time) {
            draw.push(draw_car(&self.leader, d, map));
        }
        if let Some(d) = self.follower.dist_at(time) {
            draw.push(draw_car(&self.follower, d, map));
        }
        draw
    }
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

#[derive(Clone, Debug)]
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

    fn intersection(&self, other: &Interval) -> Option<(Duration, Distance)> {
        if !overlap(
            (self.start_time, self.end_time),
            (other.start_time, other.end_time),
        ) {
            return None;
        }
        if !overlap(
            (self.start_dist, self.end_dist),
            (other.start_dist, other.end_dist),
        ) {
            return None;
        }

        // Set the two distance equations equal and solve for time. Long to type out here...
        let x1 = self.start_dist.inner_meters();
        let x2 = self.end_dist.inner_meters();
        let a1 = self.start_time.inner_seconds();
        let a2 = self.end_time.inner_seconds();

        let y1 = other.start_dist.inner_meters();
        let y2 = other.end_dist.inner_meters();
        let b1 = other.start_time.inner_seconds();
        let b2 = other.end_time.inner_seconds();

        let numer = a1 * (b2 * (y1 - x2) + b1 * (x2 - y2)) + a2 * (b2 * (x1 - y1) + b1 * (y2 - x1));
        let denom = (a1 - a2) * (y1 - y2) + b2 * (x1 - x2) + b1 * (x2 - x1);
        let t = Duration::seconds(numer / denom);

        if !self.covers(t) || !other.covers(t) {
            return None;
        }

        let dist1 = self.dist(t);
        let dist2 = other.dist(t);
        if !dist1.epsilon_eq(dist2) {
            panic!(
                "{:?} and {:?} intersect at {}, but got dist {} and {}",
                self, other, t, dist1, dist2
            );
        }
        Some((t, dist1))
    }

    // Returns the before and after interval. Both concatenated are equivalent to the original.
    fn split_at(&self, t: Duration) -> (Interval, Interval) {
        assert!(self.covers(t));
        // Maybe return optional start/end if this happens, or make the caller recognize it first.
        assert!(self.start_time != t && self.end_time != t);

        let before = Interval::new(
            self.start_dist,
            self.dist(t),
            self.start_time,
            t,
            self.start_speed,
            self.speed(t),
        );
        let after = Interval::new(
            self.dist(t),
            self.end_dist,
            t,
            self.end_time,
            self.speed(t),
            self.end_speed,
        );
        (before, after)
    }
}

// TODO use lane length and a car's properties to figure out reasonable intervals for short/long
// lanes
// TODO debug draw an interval
// TODO debug print a bunch of intervals

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

    // TODO Can we add in the following distance to our query?
    // Returns interval indices too.
    fn find_earliest_hit(&self, other: &Car) -> Option<(Duration, Distance, usize, usize)> {
        // TODO Do we ever have to worry about having the same intervals? I think this should
        // always find the earliest hit.
        // TODO A good unit test... Make sure find_hit is symmetric
        for (idx1, i1) in self.intervals.iter().enumerate() {
            for (idx2, i2) in other.intervals.iter().enumerate() {
                if let Some((time, dist)) = i1.intersection(i2) {
                    return Some((time, dist, idx1, idx2));
                }
            }
        }
        None
    }

    fn maybe_follow(&mut self, leader: &mut Car) {
        let (time, dist, idx1, idx2) = match self.find_earliest_hit(leader) {
            Some(hit) => hit,
            None => {
                return;
            }
        };
        println!(
            "Collision at {}, {}. follower interval {}, leader interval {}",
            time, dist, idx1, idx2
        );

        self.intervals.split_off(idx1 + 1);

        // TODO Might be smoother to skip this one and just go straight to adjustment.
        {
            let our_adjusted_last = self.intervals.pop().unwrap().split_at(time).0;
            self.intervals.push(our_adjusted_last);
        }

        // Match the end of the leader's interval.
        // TODO This might be too sharp a speed change. Should possibly combine this and the
        // previous interval.
        {
            let them = &leader.intervals[idx2];
            self.intervals.push(Interval::new(
                dist,
                them.end_dist,
                time,
                them.end_time,
                self.intervals.last().as_ref().unwrap().end_speed,
                them.end_speed,
            ));
        }

        // TODO What if we can't manage the same accel/deaccel/speeds?
        for i in &leader.intervals[idx2 + 1..] {
            // TODO Offset back to follow
            self.intervals.push(i.clone());
        }
    }

    fn validate(&self) {
        assert!(!self.intervals.is_empty());
        assert!(self.intervals[0].start_dist >= self.car_length);

        for pair in self.intervals.windows(2) {
            assert_eq!(pair[0].end_time, pair[1].start_time);
            assert_eq!(pair[0].end_dist, pair[1].start_dist);
            assert_eq!(pair[0].end_speed, pair[1].start_speed);
        }

        for i in &self.intervals {
            let accel = (i.end_speed - i.start_speed) / (i.end_time - i.start_time);
            if accel >= Acceleration::ZERO && accel > self.max_accel {
                println!(
                    "{} accelerates {}, but can only do {}",
                    self.id, accel, self.max_accel
                );
            }
            if accel < Acceleration::ZERO && accel < self.max_deaccel {
                println!(
                    "{} decelerates {}, but can only do {}",
                    self.id, accel, self.max_deaccel
                );
            }
        }
    }
}

fn overlap<A: PartialOrd>((a_start, a_end): (A, A), (b_start, b_end): (A, A)) -> bool {
    if a_start > b_end || b_start > a_end {
        return false;
    }
    true
}
