use crate::plugins::sim::des_model::interval::{Delta, Interval};
use geom::{Acceleration, Distance, Duration, Speed};
use map_model::{Lane, Traversable};
use sim::{CarID, CarState, DrawCarInput, VehicleType};

const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);

pub struct Car {
    pub id: CarID,
    // Hack used for different colors
    pub state: CarState,
    pub car_length: Distance,
    // TODO All of this looks jerky because we're accelerating and decelerating as fast as
    // possible!
    pub max_accel: Acceleration,
    pub max_deaccel: Acceleration,

    pub start_dist: Distance,
    pub start_time: Duration,

    // Distances represent the front of the car
    pub intervals: Vec<Interval>,
}

// Immutable public queries
impl Car {
    // None if they're not on the lane by then. Also returns the interval index for debugging.
    pub fn dist_at(&self, t: Duration) -> Option<(Distance, usize)> {
        // TODO Binary search
        for (idx, i) in self.intervals.iter().enumerate() {
            if i.covers(t) {
                return Some((i.dist(t), idx));
            }
        }
        None
    }

    pub fn validate(&self, lane: &Lane) {
        let lane_len = lane.length();
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

            if i.end_dist > lane_len {
                println!(
                    "{} ends {} past the lane end",
                    self.id,
                    i.end_dist - lane_len
                );
            }
        }
    }

    pub fn get_draw_car(&self, front: Distance, lane: &Lane) -> DrawCarInput {
        DrawCarInput {
            id: self.id,
            waiting_for_turn: None,
            stopping_trace: None,
            state: self.state,
            vehicle_type: VehicleType::Car,
            on: Traversable::Lane(lane.id),
            body: lane
                .lane_center_pts
                .slice(front - self.car_length, front)
                .unwrap()
                .0,
        }
    }

    pub fn dump_intervals(&self) {
        for i in &self.intervals {
            println!("- {}", i);
        }
    }
}

// Internal immutable math queries
impl Car {
    fn last_state(&self) -> (Distance, Speed, Duration) {
        if let Some(i) = self.intervals.last() {
            (i.end_dist, i.end_speed, i.end_time)
        } else {
            (self.start_dist, Speed::ZERO, self.start_time)
        }
    }

    fn whatif_stop_from_speed(&self, from_speed: Speed) -> Delta {
        // v_f = v_0 + a(t)
        let time_needed = -from_speed / self.max_deaccel;

        // d = (v_0)(t) + (1/2)(a)(t^2)
        let dist_covered = from_speed * time_needed
            + Distance::meters(
                0.5 * self.max_deaccel.inner_meters_per_second_squared()
                    * time_needed.inner_seconds().powi(2),
            );

        Delta::new(time_needed, dist_covered)
    }

    fn whatif_accel_from_rest(&self, to_speed: Speed) -> Delta {
        // v_f = v_0 + a(t)
        let time_needed = to_speed / self.max_accel;

        // d = (v_0)(t) + (1/2)(a)(t^2)
        // TODO Woops, don't have Duration^2
        let dist_covered = Distance::meters(
            0.5 * self.max_accel.inner_meters_per_second_squared()
                * time_needed.inner_seconds().powi(2),
        );

        Delta::new(time_needed, dist_covered)
    }

    // Returns interval indices too.
    fn find_earliest_hit(&self, leader: &Car) -> Option<(Duration, Distance, Speed, usize, usize)> {
        // TODO Do we ever have to worry about having the same intervals? I think this should
        // always find the earliest hit.
        // TODO A good unit test... Make sure find_hit is symmetric
        for (idx1, i1) in self.intervals.iter().enumerate() {
            for (idx2, i2) in leader.intervals.iter().enumerate() {
                // TODO Should bake in FOLLOWING_DISTANCE and car length!
                if let Some((time, dist, speed)) = i1.intersection(i2) {
                    return Some((time, dist, speed, idx1, idx2));
                }
            }
        }
        None
    }

    // What if we accelerate from rest, then immediately slam on the brakes, trying to cover a
    // distance. What speed should we accelerate to?
    fn find_speed_to_accel_then_asap_deaccel(&self, distance: Distance) -> Speed {
        let a = self.max_accel.inner_meters_per_second_squared();
        let b = self.max_deaccel.inner_meters_per_second_squared();
        let d = distance.inner_meters();
        let inner = (2.0 * a * b * d) / (b - a);

        if inner < 0.0 {
            panic!(
                "Can't find_speed_to_accel_then_asap_deaccel({})... sqrt of {}",
                distance, inner
            );
        }
        let result = Speed::meters_per_second(inner.sqrt());

        let actual =
            self.whatif_accel_from_rest(result).dist + self.whatif_stop_from_speed(result).dist;
        if !actual.epsilon_eq(distance) {
            panic!(
                "Wanted to cross {}, but actually would cover {}, by using {}",
                distance, actual, result
            );
        }

        result
    }
}

// Specific steps for the car to do
impl Car {
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

    pub fn accel_from_rest_to_speed_limit(&mut self, speed: Speed) {
        assert_eq!(self.last_state().1, Speed::ZERO);

        let delta = self.whatif_accel_from_rest(speed);
        self.next_state(delta.dist, speed, delta.time);
    }

    pub fn freeflow_to_cross(&mut self, dist: Distance) {
        let speed = self.last_state().1;
        assert_ne!(dist, Distance::ZERO);

        self.next_state(dist, speed, dist / speed);
    }

    pub fn deaccel_to_rest(&mut self) {
        let speed = self.last_state().1;
        assert_ne!(speed, Speed::ZERO);

        let delta = self.whatif_stop_from_speed(speed);
        self.next_state(delta.dist, Speed::ZERO, delta.time);
    }

    pub fn wait(&mut self, time: Duration) {
        let speed = self.last_state().1;
        assert_eq!(speed, Speed::ZERO);
        self.next_state(Distance::ZERO, Speed::ZERO, time);
    }
}

// Higher-level actions
impl Car {
    pub fn stop_at_end_of_lane(&mut self, lane: &Lane, speed_limit: Speed) {
        let dist_to_cover = lane.length() - self.last_state().0;

        let needed_speed = self.find_speed_to_accel_then_asap_deaccel(dist_to_cover);
        /*println!(
            "{} would need to do {} to accel then deaccel and cover lane",
            self.id, needed_speed
        );*/
        if needed_speed <= speed_limit {
            // Alright, do that then
            self.accel_from_rest_to_speed_limit(needed_speed);
            self.deaccel_to_rest();
        } else {
            /*println!(
                "  But speed limit is {}, so accel->freeflow->deaccel",
                speed_limit
            );*/
            self.accel_from_rest_to_speed_limit(speed_limit);
            let stopping_dist = self.whatif_stop_from_speed(speed_limit).dist;
            self.freeflow_to_cross(
                lane.length() - self.intervals.last().as_ref().unwrap().end_dist - stopping_dist,
            );
            self.deaccel_to_rest();
        }
    }

    pub fn maybe_follow(&mut self, leader: &mut Car) {
        let (hit_time, hit_dist, hit_speed, idx1, idx2) = match self.find_earliest_hit(leader) {
            Some(hit) => hit,
            None => {
                return;
            }
        };
        println!(
            "Collision at {}, {}, {}. follower interval {}, leader interval {}",
            hit_time, hit_dist, hit_speed, idx1, idx2
        );

        let dist_behind = leader.car_length + FOLLOWING_DISTANCE;

        self.intervals.split_off(idx1 + 1);

        // Option 1: Might be too sharp.
        if true {
            {
                let mut our_adjusted_last = self.intervals.pop().unwrap();
                our_adjusted_last.end_speed = hit_speed;
                our_adjusted_last.end_time = hit_time;
                our_adjusted_last.end_dist = hit_dist - dist_behind;
                self.intervals.push(our_adjusted_last);
            }

            {
                let them = &leader.intervals[idx2];
                self.intervals.push(Interval::new(
                    hit_dist - dist_behind,
                    them.end_dist - dist_behind,
                    hit_time,
                    them.end_time,
                    self.intervals.last().as_ref().unwrap().end_speed,
                    them.end_speed,
                ));
            }
        } else {
            // TODO This still causes impossible deaccel
            let them = &leader.intervals[idx2];
            let mut our_adjusted_last = self.intervals.pop().unwrap();
            our_adjusted_last.end_speed = them.end_speed;
            our_adjusted_last.end_time = them.end_time;
            our_adjusted_last.end_dist = them.end_dist - dist_behind;
            self.intervals.push(our_adjusted_last);
        }

        // TODO What if we can't manage the same accel/deaccel/speeds?
        for i in &leader.intervals[idx2 + 1..] {
            self.intervals.push(Interval::new(
                i.start_dist - dist_behind,
                i.end_dist - dist_behind,
                i.start_time,
                i.end_time,
                i.start_speed,
                i.end_speed,
            ));
        }
    }
}
