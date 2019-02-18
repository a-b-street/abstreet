use derive_new::new;
use geom::{Distance, Duration, Speed, EPSILON_DIST};
use std::fmt;

#[derive(Debug)]
pub struct Interval {
    pub start_dist: Distance,
    pub end_dist: Distance,
    pub start_time: Duration,
    pub end_time: Duration,
    pub start_speed: Speed,
    pub end_speed: Speed,
    // Extra info: CarID, LaneID
}

impl Interval {
    pub fn new(
        start_dist: Distance,
        end_dist: Distance,
        start_time: Duration,
        end_time: Duration,
        start_speed: Speed,
        end_speed: Speed,
    ) -> Interval {
        if start_dist < Distance::ZERO {
            panic!("Interval start_dist: {}", start_dist);
        }
        if start_time < Duration::ZERO {
            panic!("Interval start_time: {}", start_time);
        }
        // TODO And the epsilons creep in...
        if start_dist > end_dist + Distance::EPSILON {
            panic!("Interval {} .. {}", start_dist, end_dist);
        }
        if start_time >= end_time {
            panic!("Interval {} .. {}", start_time, end_time);
        }
        if start_speed < Speed::ZERO {
            panic!("Interval start_speed: {}", start_speed);
        }
        if end_speed < Speed::ZERO {
            panic!("Interval end_speed: {}", end_speed);
        }
        Interval {
            start_dist,
            end_dist,
            start_time,
            end_time,
            start_speed,
            end_speed,
        }
    }

    fn raw_accel(&self) -> f64 {
        (self.end_speed - self.start_speed).inner_meters_per_second()
            / (self.end_time - self.start_time).inner_seconds()
    }

    pub fn dist(&self, t: Duration) -> Distance {
        let relative_t = (t - self.start_time).inner_seconds();

        let d = self.start_dist.inner_meters()
            + self.start_speed.inner_meters_per_second() * relative_t
            + 0.5 * self.raw_accel() * relative_t.powi(2);
        Distance::meters(d)
    }

    pub fn speed(&self, t: Duration) -> Speed {
        // Linearly interpolate
        let result = self.start_speed + self.percent(t) * (self.end_speed - self.start_speed);
        // TODO Happens because of slight epsilonness, yay
        result.max(Speed::ZERO)
    }

    pub fn covers(&self, t: Duration) -> bool {
        t >= self.start_time - Duration::EPSILON && t <= self.end_time + Duration::EPSILON
    }

    pub fn percent(&self, t: Duration) -> f64 {
        assert!(self.covers(t));
        (t - self.start_time) / (self.end_time - self.start_time)
    }

    // Also returns the speed of self at the time of collision. Adjustments already made for slight
    // OOBness.
    pub fn intersection(&self, leader: &Interval) -> Option<(Duration, Distance, Speed)> {
        if !overlap(
            (self.start_time, self.end_time),
            (leader.start_time, leader.end_time),
        ) {
            return None;
        }
        // TODO Should bake in an epsilon check...
        if !overlap(
            (self.start_dist - EPSILON_DIST, self.end_dist + EPSILON_DIST),
            (
                leader.start_dist - EPSILON_DIST,
                leader.end_dist + EPSILON_DIST,
            ),
        ) {
            return None;
        }

        // Set the two distance equations equal and solve for time. Long to type out here...
        let t = {
            let x_1 = self.start_dist.inner_meters();
            let v_1 = self.start_speed.inner_meters_per_second();
            let t_1 = self.start_time.inner_seconds();
            let a_1 = self.raw_accel();

            let x_3 = leader.start_dist.inner_meters();
            let v_3 = leader.start_speed.inner_meters_per_second();
            let t_3 = leader.start_time.inner_seconds();
            let a_3 = leader.raw_accel();

            let q = (-0.5
                * ((2.0 * a_1 * t_1 - 2.0 * a_3 * t_3 - 2.0 * v_1 + 2.0 * v_3).powi(2)
                    - 4.0
                        * (a_3 - a_1)
                        * (-a_1 * t_1.powi(2) + a_3 * t_3.powi(2) + 2.0 * t_1 * v_1
                            - 2.0 * t_3 * v_3
                            - 2.0 * x_1
                            + 2.0 * x_3))
                    .sqrt()
                - a_1 * t_1
                + a_3 * t_3
                + v_1
                - v_3)
                / (a_3 - a_1);
            if !q.is_finite() {
                return None;
            }
            Duration::seconds(q)
        };

        if !self.covers(t) || !leader.covers(t) {
            return None;
        }

        let dist1 = self.dist(t);
        let dist2 = leader.dist(t);
        if !dist1.epsilon_eq(dist2) {
            panic!(
                "{:?} and {:?} intersect at {}, but got dist {} and {}",
                self, leader, t, dist1, dist2
            );
        }

        // Adjust solved collision distance a bit.
        Some((t, dist1.min(leader.end_dist), self.speed(t)))
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = if self.start_speed == Speed::ZERO && self.end_speed == Speed::ZERO {
            "wait"
        } else if self.start_speed == Speed::ZERO {
            "accelerate from rest"
        } else if self.end_speed == Speed::ZERO {
            "decelerate to rest"
        } else if self.start_speed == self.end_speed {
            "freeflow"
        } else if self.start_speed < self.end_speed {
            "speed up"
        } else if self.start_speed > self.end_speed {
            "slow down"
        } else {
            panic!("How to describe {:?}", self);
        };

        write!(
            f,
            "[{}] {}->{} during {}->{} ({}->{})",
            kind,
            self.start_dist,
            self.end_dist,
            self.start_time,
            self.end_time,
            self.start_speed,
            self.end_speed
        )
    }
}

// TODO debug draw an interval
// TODO debug print a bunch of intervals

fn overlap<A: PartialOrd>((a_start, a_end): (A, A), (b_start, b_end): (A, A)) -> bool {
    if a_start > b_end || b_start > a_end {
        return false;
    }
    true
}

#[derive(new)]
pub struct Delta {
    pub time: Duration,
    pub dist: Distance,
}
