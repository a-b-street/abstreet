use derive_new::new;
use geom::{Distance, Duration, Speed, EPSILON_DIST};
use std::fmt;

#[derive(Clone, Debug)]
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

    // Adjustments already made for slight OOBness.
    pub fn intersection(&self, leader: &Interval) -> Option<(Duration, Distance)> {
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
        Some((t, dist1.min(leader.end_dist)))
    }

    pub fn is_wait(&self) -> bool {
        if self.start_speed == Speed::ZERO && self.end_speed == Speed::ZERO {
            assert!(self.start_dist.epsilon_eq(self.end_dist));
            true
        } else {
            false
        }
    }

    pub fn fix_end_time(&mut self) {
        assert!(!self.is_wait());

        let g = self.end_dist.inner_meters();
        let d = self.start_dist.inner_meters();
        let v = self.start_speed.inner_meters_per_second();
        let f = self.end_speed.inner_meters_per_second();
        let s = self.start_time.inner_seconds();

        let numer = -2.0 * d + s * (f + v) + 2.0 * g;
        let denom = f + v;
        let t = Duration::seconds(numer / denom);

        self.end_time = t;
        if self.end_time <= self.start_time {
            panic!("After fixing end time, got {}", self);
        }
    }

    pub fn validate(&self, lane_len: Distance) {
        if self.start_dist < Distance::ZERO {
            panic!("Weird interval {}", self);
        }
        if self.start_time < Duration::ZERO {
            panic!("Weird interval {}", self);
        }
        // TODO And the epsilons creep in...
        if self.start_dist > self.end_dist + Distance::EPSILON {
            panic!("Weird interval {}", self);
        }
        if self.start_time >= self.end_time {
            panic!("Weird interval {}", self);
        }
        if self.start_speed < Speed::ZERO {
            panic!("Weird interval {}", self);
        }
        if self.end_speed < Speed::ZERO {
            panic!("Weird interval {}", self);
        }

        let actual_end_dist = self.dist(self.end_time);
        if !actual_end_dist.epsilon_eq(self.end_dist) {
            panic!("{} actually ends at {}", self, actual_end_dist);
        }

        if self.end_dist > lane_len + EPSILON_DIST {
            panic!(
                "{} ends {} past the lane end",
                self,
                self.end_dist - lane_len
            );
        }
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
