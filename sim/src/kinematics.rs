use dimensioned::si;
use {Acceleration, Distance, Speed, Time, TIMESTEP};

// TODO unit test all of this

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
        assert!(dist > 0.0 * si::M);

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
        assert!(current_speed <= speed_limit);
        let max_next_accel = min_accel(self.max_accel, (speed_limit - current_speed) / TIMESTEP);
        let max_next_dist = dist_at_constant_accel(max_next_accel, TIMESTEP, current_speed);
        let max_next_speed = current_speed + max_next_accel * TIMESTEP;
        max_next_dist + self.stopping_distance(max_next_speed)
    }

    fn min_next_speed(&self, current_speed: Speed) -> Speed {
        let new_speed = current_speed + self.max_deaccel * TIMESTEP;
        if new_speed >= 0.0 * si::MPS {
            return new_speed;
        }
        0.0 * si::MPS
    }

    pub fn accel_to_follow(
        &self,
        our_speed: Speed,
        other: &Vehicle,
        dist_behind_other: Distance,
        other_speed: Speed,
    ) -> Acceleration {
        // TODO this analysis isn't the same as the one in AORTA

        // What if they slam on their brakes right now?
        let their_stopping_dist = other.stopping_distance(other.min_next_speed(other_speed));
        let worst_case_dist_away = dist_behind_other + their_stopping_dist;
        self.accel_to_stop_in_dist(our_speed, worst_case_dist_away)
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
    assert!(dist >= 0.0 * si::M);
    let new_speed = initial_speed + (accel * actual_time);
    assert!(new_speed >= 0.0 * si::MPS);
    (dist, new_speed)
}
