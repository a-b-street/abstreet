use crate::runner::TestRunner;
use geom::{Acceleration, Distance, Speed, EPSILON_DIST};
use sim::kinematics::{results_of_accel_for_one_tick, Vehicle};
use sim::{Tick, TIMESTEP};

#[allow(clippy::unreadable_literal)]
pub fn run(t: &mut TestRunner) {
    // TODO table driven test style?
    /*t.run_fast("accel_to_stop_in_dist/easy", |_| {
        let v = Vehicle {
            id: CarID(0),
            debug: true,
            vehicle_type: VehicleType::Car,
            length: geom::Distance(3.0),
            max_accel: Acceleration::meters_per_second_squared(2.7),
            max_deaccel: Acceleration::meters_per_second_squared(-2.7),
            max_speed: None,
        };
        test_accel_to_stop_in_dist(v, Distance::meters(23.554161711896512), Speed::meters_per_second(8.5817572532688));
    });

    t.run_fast("accel_to_stop_in_dist/hard", |_| {
        let v = Vehicle {
            id: CarID(0),
            debug: true,
            vehicle_type: VehicleType::Car,
            length: geom::Distance(3.0),
            max_accel: Acceleration::meters_per_second_squared(2.7),
            max_deaccel: Acceleration::meters_per_second_squared(-2.7),
            max_speed: None,
        };
        test_accel_to_stop_in_dist(v, Distance::meters(4.543071997281501), Speed::meters_per_second(0.003911613164279909));
    });

    t.run_fast("accel_to_stop_in_dist/bike", |_| {
        let v = Vehicle {
            id: CarID(1481),
            debug: true,
            vehicle_type: VehicleType::Bike,
            max_accel: Acceleration::meters_per_second_squared(0.2515536204703175),
            max_deaccel: Acceleration::meters_per_second_squared(-0.23358239419143578),
            length: Distance::meters(1.9474688967345983),
            max_speed: Some(Speed::meters_per_second(4.10644207854944)),
        };
        test_accel_to_stop_in_dist(v, Distance::meters(19.34189455075048), Speed::meters_per_second(1.6099431710100307));
    });*/

    t.run_fast("time_parsing", |_| {
        assert_eq!(Tick::parse("2.3"), Some(Tick::testonly_from_raw(23)));
        assert_eq!(Tick::parse("02.3"), Some(Tick::testonly_from_raw(23)));
        assert_eq!(Tick::parse("00:00:02.3"), Some(Tick::testonly_from_raw(23)));

        assert_eq!(
            Tick::parse("00:02:03.5"),
            Some(Tick::testonly_from_raw(35 + 1200))
        );
        assert_eq!(
            Tick::parse("01:02:03.5"),
            Some(Tick::testonly_from_raw(35 + 1200 + 36000))
        );
    });

    t.run_fast("min_accel_doesnt_round_to_zero", |_| {
        // Copied from kinematics.rs, for bikes.
        let min_accel = Acceleration::meters_per_second_squared(1.1);
        let speed = min_accel * TIMESTEP;
        assert!(!speed.is_zero(TIMESTEP));
    });
}

// TODO Make sure speed never exceeds the vehicle's cap
#[allow(dead_code)]
fn test_accel_to_stop_in_dist(vehicle: Vehicle, orig_dist_left: Distance, orig_speed: Speed) {
    // Can we successfully stop in a certain distance from some initial conditions?
    let mut speed = orig_speed;
    let mut dist_left = orig_dist_left;

    for step in 0..200 {
        let desired_accel = vehicle.accel_to_stop_in_dist(speed, dist_left).unwrap();
        let accel = vehicle.clamp_accel(desired_accel);
        println!(
            "Step {}: speed {}, dist_left {}, want accel {} but doing {}",
            step, speed, dist_left, desired_accel, accel
        );

        let (dist_covered, new_speed) = results_of_accel_for_one_tick(speed, accel);
        speed = new_speed;
        dist_left -= dist_covered;

        if dist_left < -EPSILON_DIST {
            println!("  Result: speed {}, dist_left {}", speed, dist_left);
            panic!("We overshot too much!");
        }
        if dist_left <= EPSILON_DIST {
            println!("  Result: speed {}, dist_left {}", speed, dist_left);
            if !speed.is_zero(TIMESTEP) {
                panic!("Finished, but going too fast");
            }
            return;
        }
    }
    println!("  Result: speed {}, dist_left {}", speed, dist_left);
    panic!(
        "Didn't finish in 20s; only covered {} of {}",
        orig_dist_left - dist_left,
        orig_dist_left
    );
}
