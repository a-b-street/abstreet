use dimensioned::si;
use {Acceleration, Distance, Speed, Time, TIMESTEP};

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
}

fn dist_at_constant_accel(accel: Acceleration, time: Time, initial_speed: Speed) -> Distance {
    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= 0.0 * si::MPS2 {
        time
    } else {
        min(time, -1.0 * initial_speed / accel)
    };
    (initial_speed * actual_time) + (0.5 * accel * (actual_time * actual_time))
}

fn min(t1: Time, t2: Time) -> Time {
    if t1 < t2 {
        return t1;
    }
    t2
}

// TODO combine these two
pub fn new_speed_after_tick(speed: Speed, accel: Acceleration) -> Speed {
    // Don't deaccelerate past 0
    let new_speed = speed + (accel * TIMESTEP);
    if new_speed >= 0.0 * si::MPS {
        return new_speed;
    }
    0.0 * si::MPS
}

pub fn dist_at_constant_accel_for_one_tick(accel: Acceleration, initial_speed: Speed) -> Distance {
    // TODO impl this
    return 0.0 * si::M;
}
