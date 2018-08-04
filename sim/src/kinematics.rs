use dimensioned::si;
use TIMESTEP;

pub struct Vehicle {
    // > 0
    max_accel: si::MeterPerSecond2<f64>,
    // < 0
    max_deaccel: si::MeterPerSecond2<f64>,
}

impl Vehicle {
    pub fn typical_car() -> Vehicle {
        Vehicle {
            max_accel: 2.7 * si::MPS2,
            max_deaccel: -2.7 * si::MPS2,
        }
    }

    pub fn stopping_distance(&self, speed: si::MeterPerSecond<f64>) -> si::Meter<f64> {
        // v_f = v_0 + a*t
        // TODO why isn't this part negative?
        let stopping_time = speed / self.max_accel;
        dist_at_constant_accel(self.max_deaccel, stopping_time, speed)
    }

    pub fn accel_to_achieve_speed_in_one_tick(
        &self,
        current: si::MeterPerSecond<f64>,
        target: si::MeterPerSecond<f64>,
    ) -> si::MeterPerSecond2<f64> {
        (target - current) / TIMESTEP
    }
}

fn dist_at_constant_accel(
    accel: si::MeterPerSecond2<f64>,
    time: si::Second<f64>,
    initial_speed: si::MeterPerSecond<f64>,
) -> si::Meter<f64> {
    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= 0.0 * si::MPS2 {
        time
    } else {
        min(time, -1.0 * initial_speed / accel)
    };
    (initial_speed * actual_time) + (0.5 * accel * (actual_time * actual_time))
}

fn min(t1: si::Second<f64>, t2: si::Second<f64>) -> si::Second<f64> {
    if t1 < t2 {
        return t1;
    }
    t2
}

// TODO combine these two
pub fn new_speed_after_tick(
    speed: si::MeterPerSecond<f64>,
    accel: si::MeterPerSecond2<f64>,
) -> si::MeterPerSecond<f64> {
    // Don't deaccelerate past 0
    let new_speed = speed + (accel * TIMESTEP);
    if new_speed >= 0.0 * si::MPS {
        return new_speed;
    }
    0.0 * si::MPS
}

pub fn dist_at_constant_accel_for_one_tick(
    accel: si::MeterPerSecond2<f64>,
    initial_speed: si::MeterPerSecond<f64>,
) -> si::Meter<f64> {
    // TODO impl this
    return 0.0 * si::M;
}
