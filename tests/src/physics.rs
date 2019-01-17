use crate::runner::TestRunner;
use dimensioned::si;
use geom::EPSILON_DIST;
use sim::kinematics::{results_of_accel_for_one_tick, Vehicle, EPSILON_SPEED};
use sim::{CarID, Distance, Speed, Tick, VehicleType};

#[allow(clippy::unreadable_literal)]
pub fn run(t: &mut TestRunner) {
    // TODO table driven test style?
    t.run_fast("accel_to_stop_in_dist/easy", |_| {
        let v = Vehicle {
            id: CarID(0),
            debug: true,
            vehicle_type: VehicleType::Car,
            length: 3.0 * si::M,
            max_accel: 2.7 * si::MPS2,
            max_deaccel: -2.7 * si::MPS2,
            max_speed: None,
        };
        test_accel_to_stop_in_dist(v, 23.554161711896512 * si::M, 8.5817572532688 * si::MPS);
    });

    t.run_fast("accel_to_stop_in_dist/hard", |_| {
        let v = Vehicle {
            id: CarID(0),
            debug: true,
            vehicle_type: VehicleType::Car,
            length: 3.0 * si::M,
            max_accel: 2.7 * si::MPS2,
            max_deaccel: -2.7 * si::MPS2,
            max_speed: None,
        };
        test_accel_to_stop_in_dist(v, 4.543071997281501 * si::M, 0.003911613164279909 * si::MPS);
    });

    t.run_fast("accel_to_stop_in_dist/bike", |_| {
        let v = Vehicle {
            id: CarID(1481),
            debug: true,
            vehicle_type: VehicleType::Bike,
            max_accel: 0.2515536204703175 * si::MPS2,
            max_deaccel: -0.23358239419143578 * si::MPS2,
            length: 1.9474688967345983 * si::M,
            max_speed: Some(4.10644207854944 * si::MPS),
        };
        test_accel_to_stop_in_dist(v, 19.34189455075048 * si::M, 1.6099431710100307 * si::MPS);
    });

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
}

// TODO Make sure speed never exceeds the vehicle's cap
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
            if speed > EPSILON_SPEED {
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
