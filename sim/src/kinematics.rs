use dimensioned::si;
use geom::EPSILON_DIST;
use models::FOLLOWING_DISTANCE;
use std;
use {Acceleration, Distance, Speed, Time, TIMESTEP};

pub const EPSILON_SPEED: Speed = si::MeterPerSecond {
    value_unsafe: 0.00000001,
    _marker: std::marker::PhantomData,
};

// TODO unit test all of this
// TODO handle floating point issues uniformly here

pub struct Vehicle {
    // > 0
    max_accel: Acceleration,
    // < 0
    max_deaccel: Acceleration,
}

impl Vehicle {
    pub fn typical_car() -> Vehicle {
        Vehicle {
            max_accel: 2.7 * si::MPS2,
            max_deaccel: -2.7 * si::MPS2,
        }
    }

    pub fn clamp_accel(&self, accel: Acceleration) -> Acceleration {
        if accel < self.max_deaccel {
            self.max_deaccel
        } else if accel > self.max_accel {
            self.max_accel
        } else {
            accel
        }
    }

    pub fn stopping_distance(&self, speed: Speed) -> Distance {
        // v_f = v_0 + a*t
        // TODO why isn't this part negative?
        let stopping_time = speed / self.max_accel;
        dist_at_constant_accel(self.max_deaccel, stopping_time, speed)
    }

    pub fn accel_to_achieve_speed_in_one_tick(
        &self,
        current: Speed,
        target: Speed,
    ) -> Acceleration {
        (target - current) / TIMESTEP
    }

    // TODO this needs unit tests and some careful checking
    pub fn accel_to_stop_in_dist(&self, speed: Speed, dist: Distance) -> Acceleration {
        assert_ge!(dist, -EPSILON_DIST);
        // Don't NaN out. Don't check for <= EPSILON_DIST here -- it makes cars slightly overshoot
        // sometimes.
        if dist <= 0.0 * si::M {
            return 0.0 * si::MPS2;
        }

        // d = (v_1)(t) + (1/2)(a)(t^2)
        // 0 = (v_1) + (a)(t)
        // Eliminating time yields the formula for accel below. This same accel should be applied
        // for t = -v_1 / a, which is possible even if that's not a multiple of TIMESTEP since
        // we're decelerating to rest.
        let normal_case: Acceleration = (-1.0 * speed * speed) / (2.0 * dist);
        let required_time: Time = -1.0 * speed / normal_case;

        if !required_time.value_unsafe.is_nan() {
            return normal_case;
        }

        // We have to accelerate so that we can get going, but not enough so that we can't stop. Do
        // one tick of acceleration, one tick of deacceleration at that same rate. If the required
        // acceleration is then too high, we'll cap off and trigger a normal case next tick.
        // Want (1/2)(a)(dt^2) + (a dt)dt - (1/2)(a)(dt^2) = dist
        dist / (TIMESTEP * TIMESTEP)
    }

    // Assume we accelerate as much as possible this tick (restricted only by the speed limit),
    // then stop as fast as possible.
    pub fn max_lookahead_dist(&self, current_speed: Speed, speed_limit: Speed) -> Distance {
        assert_le!(current_speed, speed_limit);
        let max_next_accel = min_accel(self.max_accel, (speed_limit - current_speed) / TIMESTEP);
        let max_next_dist = dist_at_constant_accel(max_next_accel, TIMESTEP, current_speed);
        let max_next_speed = current_speed + max_next_accel * TIMESTEP;
        max_next_dist + self.stopping_distance(max_next_speed)
    }

    // TODO share with max_lookahead_dist
    fn max_next_dist(&self, current_speed: Speed, speed_limit: Speed) -> Distance {
        assert_le!(current_speed, speed_limit);
        let max_next_accel = min_accel(self.max_accel, (speed_limit - current_speed) / TIMESTEP);
        dist_at_constant_accel(max_next_accel, TIMESTEP, current_speed)
    }

    /*fn min_next_speed(&self, current_speed: Speed) -> Speed {
        let new_speed = current_speed + self.max_deaccel * TIMESTEP;
        if new_speed >= 0.0 * si::MPS {
            return new_speed;
        }
        0.0 * si::MPS
    }*/

    fn min_next_dist(&self, current_speed: Speed) -> Distance {
        dist_at_constant_accel(self.max_deaccel, TIMESTEP, current_speed)
    }

    pub fn accel_to_follow(
        &self,
        our_speed: Speed,
        our_speed_limit: Speed,
        other: &Vehicle,
        dist_behind_other: Distance,
        other_speed: Speed,
    ) -> Acceleration {
        /* A seemingly failed attempt at a simpler version:

        // What if they slam on their brakes right now?
        let their_stopping_dist = other.stopping_distance(other.min_next_speed(other_speed));
        let worst_case_dist_away = dist_behind_other + their_stopping_dist;
        self.accel_to_stop_in_dist(our_speed, worst_case_dist_away)
        */

        let us_worst_dist = self.max_lookahead_dist(our_speed, our_speed_limit);
        let most_we_could_go = self.max_next_dist(our_speed, our_speed_limit);
        let least_they_could_go = other.min_next_dist(other_speed);

        // TODO this optimizes for next tick, so we're playing it really
        // conservative here... will that make us fluctuate more?
        let projected_dist_from_them = dist_behind_other - most_we_could_go + least_they_could_go;
        let desired_dist_btwn = us_worst_dist + FOLLOWING_DISTANCE;

        // Positive = speed up, zero = go their speed, negative = slow down
        let delta_dist = projected_dist_from_them - desired_dist_btwn;

        // Try to cover whatever the distance is
        accel_to_cover_dist_in_one_tick(delta_dist, our_speed)
    }
}

fn dist_at_constant_accel(accel: Acceleration, time: Time, initial_speed: Speed) -> Distance {
    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= 0.0 * si::MPS2 {
        time
    } else {
        min_time(time, -1.0 * initial_speed / accel)
    };
    (initial_speed * actual_time) + (0.5 * accel * (actual_time * actual_time))
}

fn min_time(t1: Time, t2: Time) -> Time {
    if t1 < t2 {
        return t1;
    }
    t2
}

fn min_accel(a1: Acceleration, a2: Acceleration) -> Acceleration {
    if a1 < a2 {
        return a1;
    }
    a2
}

pub fn results_of_accel_for_one_tick(
    initial_speed: Speed,
    accel: Acceleration,
) -> (Distance, Speed) {
    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= 0.0 * si::MPS2 {
        TIMESTEP
    } else {
        min_time(TIMESTEP, -1.0 * initial_speed / accel)
    };
    let dist = (initial_speed * actual_time) + (0.5 * accel * (actual_time * actual_time));
    assert_ge!(dist, 0.0 * si::M);
    let mut new_speed = initial_speed + (accel * actual_time);
    // Handle some floating point imprecision
    if new_speed < 0.0 * si::MPS && new_speed >= -1.0 * EPSILON_SPEED {
        new_speed = 0.0 * si::MPS;
    }
    assert_ge!(new_speed, 0.0 * si::MPS);
    (dist, new_speed)
}

fn accel_to_cover_dist_in_one_tick(dist: Distance, speed: Speed) -> Acceleration {
    // d = (v_i)(t) + (1/2)(a)(t^2), solved for a
    2.0 * (dist - (speed * TIMESTEP)) / (TIMESTEP * TIMESTEP)
}
