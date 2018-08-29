extern crate dimensioned;
extern crate geom;
extern crate sim;

use dimensioned::si;
use geom::EPSILON_DIST;
use sim::kinematics::{results_of_accel_for_one_tick, Vehicle, EPSILON_SPEED};
use sim::{CarID, Distance, Speed};

// TODO table driven test style?

#[test]
fn test_accel_to_stop_in_dist_easy() {
    test_accel_to_stop_in_dist(23.554161711896512 * si::M, 8.5817572532688 * si::MPS)
}

#[test]
fn test_accel_to_stop_in_dist_hard() {
    test_accel_to_stop_in_dist(4.543071997281501 * si::M, 0.003911613164279909 * si::MPS);
}

fn test_accel_to_stop_in_dist(orig_dist_left: Distance, orig_speed: Speed) {
    let vehicle = Vehicle {
        id: CarID(0),
        length: 3.0 * si::M,
        max_accel: 2.7 * si::MPS2,
        max_deaccel: -2.7 * si::MPS2,
    };

    // Can we successfully stop in a certain distance from some initial conditions?
    let mut speed = orig_speed;
    let mut dist_left = orig_dist_left;

    for step in 0..100 {
        let desired_accel = vehicle.accel_to_stop_in_dist(speed, dist_left).unwrap();
        let accel = vehicle.clamp_accel(desired_accel);
        println!(
            "Step {}: speed {}, dist_left {}, want accel {} but doing {}",
            step, speed, dist_left, desired_accel, accel
        );

        let (dist_covered, new_speed) = results_of_accel_for_one_tick(speed, accel);
        speed = new_speed;
        dist_left -= dist_covered;
        println!("  Result: speed {}, dist_left {}", speed, dist_left);

        if dist_left < -EPSILON_DIST {
            panic!("We overshot too much!");
        }
        if dist_left <= EPSILON_DIST {
            if speed > EPSILON_SPEED {
                panic!("Finished, but going too fast");
            }
            return;
        }
    }
    panic!("Didn't finish; only covered {}", orig_dist_left - dist_left);
}
