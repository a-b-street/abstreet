use crate::{CarID, TIMESTEP};
use abstutil::Error;
use geom::{Acceleration, Distance, Duration, Speed, EPSILON_DIST};
use more_asserts::assert_ge;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};

// http://pccsc.net/bicycle-parking-info/ says 68 inches, which is 1.73m
const MIN_BIKE_LENGTH: Distance = Distance::const_meters(1.7);
pub const MAX_BIKE_LENGTH: Distance = Distance::const_meters(2.0);
// These two must be < PARKING_SPOT_LENGTH
const MIN_CAR_LENGTH: Distance = Distance::const_meters(4.5);
pub const MAX_CAR_LENGTH: Distance = Distance::const_meters(6.5);
// Note this is more than MAX_CAR_LENGTH
pub const BUS_LENGTH: Distance = Distance::const_meters(12.5);

// At all speeds (including at rest), cars must be at least this far apart, measured from front of
// one car to the back of the other.
pub const FOLLOWING_DISTANCE: Distance = Distance::const_meters(1.0);

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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
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
            max_accel: Acceleration::meters_per_second_squared(rng.gen_range(2.4, 2.8)),
            max_deaccel: Acceleration::meters_per_second_squared(rng.gen_range(-2.8, -2.4)),
            // TODO more realistic to have a few preset lengths and choose between them
            length: Distance::meters(
                rng.gen_range(MIN_CAR_LENGTH.inner_meters(), MAX_CAR_LENGTH.inner_meters()),
            ),
            max_speed: None,
        }
    }

    pub fn generate_bus(id: CarID, rng: &mut XorShiftRng) -> Vehicle {
        Vehicle {
            id,
            vehicle_type: VehicleType::Bus,
            debug: false,
            max_accel: Acceleration::meters_per_second_squared(rng.gen_range(2.4, 2.8)),
            max_deaccel: Acceleration::meters_per_second_squared(rng.gen_range(-2.8, -2.4)),
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
            // TODO But it's too slow, bikes can't accelerate past a non-zeroish speed in 0.1s.
            // Workaround properly... perhaps with a continuous time approach.
            max_accel: Acceleration::meters_per_second_squared(rng.gen_range(1.1, 1.3)),
            // Much easier deaccel. Partly to avoid accel_to_stop_in_dist bugs with bikes running
            // stop signs.
            max_deaccel: Acceleration::meters_per_second_squared(rng.gen_range(-1.3, -1.2)),
            length: Distance::meters(rng.gen_range(
                MIN_BIKE_LENGTH.inner_meters(),
                MAX_BIKE_LENGTH.inner_meters(),
            )),
            // 7 to 10 mph
            max_speed: Some(Speed::meters_per_second(rng.gen_range(3.13, 4.47))),
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
        let stopping_time = speed / self.max_deaccel * -1.0;
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
        if dist <= Distance::ZERO {
            // TODO assert speed is 0ish?
            return Ok(Acceleration::ZERO);
        }

        // d = (v_1)(t) + (1/2)(a)(t^2)
        // 0 = (v_1) + (a)(t)
        // Eliminating time yields the formula for accel below. This same accel should be applied
        // for t = -v_1 / a, which is possible even if that's not a multiple of TIMESTEP since
        // we're decelerating to rest.
        let normal_case = Acceleration::meters_per_second_squared(
            (-1.0 * speed.inner_meters_per_second() * speed.inner_meters_per_second())
                / (2.0 * dist.inner_meters()),
        );
        // TODO might validlyish be NaN, so just f64 here
        let required_time =
            -1.0 * speed.inner_meters_per_second() / normal_case.inner_meters_per_second_squared();

        if self.debug {
            println!(
                "   accel_to_stop_in_dist({}, {}) would normally recommend {} and take {}s to finish",
                speed, dist, normal_case, required_time
            );
        }

        // TODO If we don't restrict required_time from growing arbitrarily high, then it takes an
        // absurd amount of time to finish, with tiny little steps. But need to tune and understand
        // this value better. Higher initial speeds or slower max_deaccel's mean this is naturally
        // going to take longer. We don't want to start stopping now if we can't undo it next tick.
        if required_time.is_finite() && Duration::seconds(required_time) < Duration::seconds(15.0) {
            return Ok(normal_case);
        }

        // We have to accelerate so that we can get going, but not enough so that we can't stop. Do
        // one tick of acceleration, one tick of deacceleration at that same rate. If the required
        // acceleration is then too high, we'll cap off and trigger a normal case next tick.
        // Want (1/2)(a)(dt^2) + (a dt)dt - (1/2)(a)(dt^2) = dist
        // TODO I don't understand the above.
        Ok(Acceleration::meters_per_second_squared(
            dist.inner_meters() / (TIMESTEP.inner_seconds() * TIMESTEP.inner_seconds()),
        ))
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

        Ok(self
            .max_accel
            .min(self.accel_to_achieve_speed_in_one_tick(current_speed, speed_limit)))
    }

    fn max_next_dist(&self, current_speed: Speed, speed_limit: Speed) -> Result<Distance, Error> {
        let max_next_accel = self.max_next_accel(current_speed, speed_limit)?;
        dist_at_constant_accel(max_next_accel, TIMESTEP, current_speed)
    }

    fn min_next_dist(&self, current_speed: Speed) -> Result<Distance, Error> {
        dist_at_constant_accel(self.max_deaccel, TIMESTEP, current_speed)
    }

    pub fn accel_to_follow(
        &self,
        our_speed: Speed,
        our_speed_limit: Speed,
        other: &Vehicle,
        dist_behind_others_back: Distance,
        other_speed: Speed,
    ) -> Result<Acceleration, Error> {
        let us_worst_dist = self.max_lookahead_dist(our_speed, our_speed_limit)?;
        let most_we_could_go = self.max_next_dist(our_speed, our_speed_limit)?;
        let least_they_could_go = other.min_next_dist(other_speed)?;

        // TODO this optimizes for next tick, so we're playing it really
        // conservative here... will that make us fluctuate more?
        let projected_dist_from_them =
            dist_behind_others_back - most_we_could_go + least_they_could_go;
        let desired_dist_btwn = us_worst_dist + FOLLOWING_DISTANCE;

        // Positive = speed up, zero = go their speed, negative = slow down
        let delta_dist = projected_dist_from_them - desired_dist_btwn;

        // Try to cover whatever the distance is
        Ok(accel_to_cover_dist_in_one_tick(delta_dist, our_speed))
    }
}

fn dist_at_constant_accel(
    accel: Acceleration,
    time: Duration,
    initial_speed: Speed,
) -> Result<Distance, Error> {
    if time < Duration::ZERO {
        return Err(Error::new(format!(
            "dist_at_constant_accel called with time = {}",
            time
        )));
    }

    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= Acceleration::ZERO {
        time
    } else {
        // 0 = v_0 + a*t
        time.min(initial_speed / accel * -1.0)
    };
    let dist = (initial_speed * actual_time)
        + Distance::meters(
            0.5 * accel.inner_meters_per_second_squared()
                * (actual_time.inner_seconds() * actual_time.inner_seconds()),
        );
    if dist < Distance::ZERO {
        return Err(Error::new(format!(
            "dist_at_constant_accel yielded result = {}",
            dist
        )));
    }
    Ok(dist)
}

pub fn results_of_accel_for_one_tick(
    initial_speed: Speed,
    accel: Acceleration,
) -> (Distance, Speed) {
    // Don't deaccelerate into going backwards, just cap things off.
    let actual_time = if accel >= Acceleration::ZERO {
        TIMESTEP
    } else {
        TIMESTEP.min(initial_speed / accel * -1.0)
    };
    let dist = (initial_speed * actual_time)
        + Distance::meters(
            0.5 * accel.inner_meters_per_second_squared()
                * (actual_time.inner_seconds() * actual_time.inner_seconds()),
        );
    assert_ge!(dist, Distance::ZERO);
    let new_speed = initial_speed + (accel * actual_time);
    // Deal with floating point imprecision.
    if new_speed.is_zero(TIMESTEP) {
        (dist, Speed::ZERO)
    } else {
        assert_ge!(new_speed, Speed::ZERO);
        (dist, new_speed)
    }
}

fn accel_to_cover_dist_in_one_tick(dist: Distance, speed: Speed) -> Acceleration {
    // d = (v_i)(t) + (1/2)(a)(t^2), solved for a
    Acceleration::meters_per_second_squared(
        2.0 * (dist - (speed * TIMESTEP)).inner_meters()
            / (TIMESTEP.inner_seconds() * TIMESTEP.inner_seconds()),
    )
}
