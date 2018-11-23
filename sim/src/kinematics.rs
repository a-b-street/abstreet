use abstutil::Error;
use dimensioned::si;
use geom::EPSILON_DIST;
use rand::{Rng, XorShiftRng};
use std;
use {Acceleration, CarID, Distance, Speed, Time, TIMESTEP};

pub const EPSILON_SPEED: Speed = si::MeterPerSecond {
    value_unsafe: 0.00000001,
    _marker: std::marker::PhantomData,
};

// http://pccsc.net/bicycle-parking-info/ says 68 inches, which is 1.73m
const MIN_BIKE_LENGTH: Distance = si::Meter {
    value_unsafe: 1.7,
    _marker: std::marker::PhantomData,
};
const MAX_BIKE_LENGTH: Distance = si::Meter {
    value_unsafe: 2.0,
    _marker: std::marker::PhantomData,
};
// These two must be < PARKING_SPOT_LENGTH
const MIN_CAR_LENGTH: Distance = si::Meter {
    value_unsafe: 4.5,
    _marker: std::marker::PhantomData,
};
const MAX_CAR_LENGTH: Distance = si::Meter {
    value_unsafe: 6.5,
    _marker: std::marker::PhantomData,
};
// Note this is more than MAX_CAR_LENGTH
const BUS_LENGTH: Distance = si::Meter {
    value_unsafe: 12.5,
    _marker: std::marker::PhantomData,
};

// At all speeds (including at rest), cars must be at least this far apart, measured from front of
// one car to the back of the other.
const FOLLOWING_DISTANCE: Distance = si::Meter {
    value_unsafe: 1.0,
    _marker: std::marker::PhantomData,
};

// TODO unit test all of this
// TODO handle floating point issues uniformly here

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: CarID,
    pub vehicle_type: VehicleType,
    pub debug: bool,

    // > 0
    pub max_accel: Acceleration,
    // < 0
    pub max_deaccel: Acceleration,

    pub length: Distance,
    pub max_speed: Option<Speed>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VehicleType {
    Car,
    Bus,
    Bike,
}

impl Vehicle {
    pub fn generate_car(id: CarID, rng: &mut XorShiftRng) -> Vehicle {
        Vehicle {
            id,
            vehicle_type: VehicleType::Car,
            debug: false,
            max_accel: rng.gen_range(2.4, 2.8) * si::MPS2,
            max_deaccel: rng.gen_range(-2.8, -2.4) * si::MPS2,
            // TODO more realistic to have a few preset lengths and choose between them
            length: rng.gen_range(MIN_CAR_LENGTH.value_unsafe, MAX_CAR_LENGTH.value_unsafe) * si::M,
            max_speed: None,
        }
    }

    pub fn generate_bus(id: CarID, rng: &mut XorShiftRng) -> Vehicle {
        Vehicle {
            id,
            vehicle_type: VehicleType::Bus,
            debug: false,
            max_accel: rng.gen_range(2.4, 2.8) * si::MPS2,
            max_deaccel: rng.gen_range(-2.8, -2.4) * si::MPS2,
            length: BUS_LENGTH,
            max_speed: None,
        }
    }

    pub fn generate_bike(id: CarID, rng: &mut XorShiftRng) -> Vehicle {
        Vehicle {
            id,
            vehicle_type: VehicleType::Bike,
            debug: false,
            // http://eprints.uwe.ac.uk/20767/ says mean 0.231
            max_accel: rng.gen_range(0.2, 0.3) * si::MPS2,
            // Much easier deaccel. Partly to avoid accel_to_stop_in_dist bugs with bikes running
            // stop signs.
            max_deaccel: rng.gen_range(-1.3, -1.2) * si::MPS2,
            length: rng.gen_range(MIN_BIKE_LENGTH.value_unsafe, MAX_BIKE_LENGTH.value_unsafe)
                * si::M,
            // 7 to 10 mph
            max_speed: Some(rng.gen_range(3.13, 4.47) * si::MPS),
        }
    }

    // At rest, used to determine max capacity of SimQueue
    pub fn best_case_following_dist() -> Distance {
        MIN_BIKE_LENGTH + FOLLOWING_DISTANCE
    }

    pub fn worst_case_following_dist() -> Distance {
        BUS_LENGTH + FOLLOWING_DISTANCE
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

    pub fn clamp_speed(&self, speed: Speed) -> Speed {
        if let Some(limit) = self.max_speed {
            if speed > limit {
                return limit;
            }
        }
        speed
    }

    pub fn stopping_distance(&self, speed: Speed) -> Result<Distance, Error> {
        // 0 = v_0 + a*t
        let stopping_time = -1.0 * speed / self.max_deaccel;
        dist_at_constant_accel(self.max_deaccel, stopping_time, speed)
    }

    pub fn accel_to_achieve_speed_in_one_tick(
        &self,
        current: Speed,
        target: Speed,
    ) -> Acceleration {
        // v_f = v_0 + a*t
        (target - current) / TIMESTEP
    }

    // TODO this needs unit tests and some careful checking
    pub fn accel_to_stop_in_dist(
        &self,
        speed: Speed,
        dist: Distance,
    ) -> Result<Acceleration, Error> {
        if dist < -EPSILON_DIST {
            return Err(Error::new(format!(
                "{} called accel_to_stop_in_dist({}, {}) with negative distance",
                self.id, speed, dist
            )));
        }

        // Don't NaN out. Don't check for <= EPSILON_DIST here -- it makes cars slightly overshoot
        // sometimes.
        if dist <= 0.0 * si::M {
            // TODO assert speed is 0ish?
            return Ok(0.0 * si::MPS2);
        }

        // d = (v_1)(t) + (1/2)(a)(t^2)
        // 0 = (v_1) + (a)(t)
        // Eliminating time yields the formula for accel below. This same accel should be applied
        // for t = -v_1 / a, which is possible even if that's not a multiple of TIMESTEP since
        // we're decelerating to rest.
        let normal_case: Acceleration = (-1.0 * speed * speed) / (2.0 * dist);
        let required_time: Time = -1.0 * speed / normal_case;

        if self.debug {
            debug!(
                "   accel_to_stop_in_dist({}, {}) would normally recommend {} and take {} to finish",
                speed, dist, normal_case, required_time
            );
        }

        // TODO If we don't restrict required_time from growing arbitrarily high, then it takes an
        // absurd amount of time to finish, with tiny little steps. But need to tune and understand
        // this value better. Higher initial speeds or slower max_deaccel's mean this is naturally
        // going to take longer. We don't want to start stopping now if we can't undo it next tick.
        if !required_time.value_unsafe.is_nan() && required_time < 15.0 * si::S {
            return Ok(normal_case);
        }

        // We have to accelerate so that we can get going, but not enough so that we can't stop. Do
        // one tick of acceleration, one tick of deacceleration at that same rate. If the required
        // acceleration is then too high, we'll cap off and trigger a normal case next tick.
        // Want (1/2)(a)(dt^2) + (a dt)dt - (1/2)(a)(dt^2) = dist
        // TODO I don't understand the above.
        Ok(dist / (TIMESTEP * TIMESTEP))
    }

    // Assume we accelerate as much as possible this tick (restricted only by the speed limit),
    // then stop as fast as possible.
    pub fn max_lookahead_dist(
        &self,
        current_speed: Speed,
        speed_limit: Speed,
    ) -> Result<Distance, Error> {
        let max_next_accel = self.max_next_accel(current_speed, speed_limit)?;
        let max_next_dist = self.max_next_dist(current_speed, speed_limit)?;
        let max_next_speed = current_speed + max_next_accel * TIMESTEP;
        Ok(max_next_dist + self.stopping_distance(max_next_speed)?)
    }

    fn max_next_accel(
        &self,
        current_speed: Speed,
        speed_limit: Speed,
    ) -> Result<Acceleration, Error> {
        if current_speed > speed_limit {
            return Err(Error::new(format!(
                "{} called max_lookahead_dist({}, {}) with current speed over the limit",
                self.id, current_speed, speed_limit
            )));
        }

        Ok(min_accel(
            self.max_accel,
            self.accel_to_achieve_speed_in_one_tick(current_speed, speed_limit),
        ))
    }

    fn max_next_dist(&self, current_speed: Speed, speed_limit: Speed) -> Result<Distance, Error> {
        let max_next_accel = self.max_next_accel(current_speed, speed_limit)?;
        dist_at_constant_accel(max_next_accel, TIMESTEP, current_speed)
    }

    fn min_next_dist(&self, current_speed: Speed) -> Result<Distance, Error> {
        dist_at_constant_accel(self.max_deaccel, TIMESTEP, current_speed)
    }

    // Relative to the front of the car
    pub fn following_dist(&self) -> Distance {
        self.length + FOLLOWING_DISTANCE
    }

    pub fn accel_to_follow(
        &self,
        our_speed: Speed,
        our_speed_limit: Speed,
        other: &Vehicle,
        dist_behind_other: Distance,
        other_speed: Speed,
    ) -> Result<Acceleration, Error> {
        /* A seemingly failed attempt at a simpler version:

        // What if they slam on their brakes right now?
        let their_stopping_dist = other.stopping_distance(other.min_next_speed(other_speed));
        let worst_case_dist_away = dist_behind_other + their_stopping_dist;
        self.accel_to_stop_in_dist(our_speed, worst_case_dist_away)
        */

        let us_worst_dist = self.max_lookahead_dist(our_speed, our_speed_limit)?;
        let most_we_could_go = self.max_next_dist(our_speed, our_speed_limit)?;
        let least_they_could_go = other.min_next_dist(other_speed)?;

        // TODO this optimizes for next tick, so we're playing it really
        // conservative here... will that make us fluctuate more?
        let projected_dist_from_them = dist_behind_other - most_we_could_go + least_they_could_go;
        let desired_dist_btwn = us_worst_dist + other.following_dist();

        // Positive = speed up, zero = go their speed, negative = slow down
        let delta_dist = projected_dist_from_them - desired_dist_btwn;

        // Try to cover whatever the distance is
        Ok(accel_to_cover_dist_in_one_tick(delta_dist, our_speed))
    }
}

fn dist_at_constant_accel(
    accel: Acceleration,
    time: Time,
    initial_speed: Speed,
) -> Result<Distance, Error> {
    if time < 0.0 * si::S {
        return Err(Error::new(format!(
            "dist_at_constant_accel called with time = {}",
            time
        )));
    }

    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= 0.0 * si::MPS2 {
        time
    } else {
        // 0 = v_0 + a*t
        min_time(time, -1.0 * initial_speed / accel)
    };
    let dist = (initial_speed * actual_time) + (0.5 * accel * (actual_time * actual_time));
    if dist < 0.0 * si::M {
        return Err(Error::new(format!(
            "dist_at_constant_accel yielded result = {}",
            dist
        )));
    }
    Ok(dist)
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
