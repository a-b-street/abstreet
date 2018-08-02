use dimensioned::si;

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
