use derive_new::new;
use geom::{Distance, Duration, Speed};

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
        assert!(start_dist >= Distance::ZERO);
        assert!(start_time >= Duration::ZERO);
        assert!(start_dist <= end_dist);
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

    pub fn dist(&self, t: Duration) -> Distance {
        // Linearly interpolate
        self.start_dist + self.percent(t) * (self.end_dist - self.start_dist)
    }

    pub fn speed(&self, t: Duration) -> Speed {
        // Linearly interpolate
        self.start_speed + self.percent(t) * (self.end_speed - self.start_speed)
    }

    pub fn covers(&self, t: Duration) -> bool {
        t >= self.start_time && t <= self.end_time
    }

    pub fn percent(&self, t: Duration) -> f64 {
        assert!(self.covers(t));
        (t - self.start_time) / (self.end_time - self.start_time)
    }

    pub fn intersection(&self, other: &Interval) -> Option<(Duration, Distance)> {
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
    /*fn split_at(&self, t: Duration) -> (Interval, Interval) {
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
    }*/
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
