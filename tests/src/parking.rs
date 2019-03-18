use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use sim::{DrivingGoal, Event, ParkingSpot, SidewalkSpot, SimFlags, TripSpec};

pub fn run(t: &mut TestRunner) {
    // TODO Lots of boilerplate between these two. Can we do better?

    t.run_slow("park_on_goal_st", |h| {
        let (map, mut sim, mut rng) = SimFlags::synthetic_test("parking_test", "park_on_goal_st")
            .load(None, &mut Timer::throwaway());
        let north_bldg = map.bldg("north").id;
        let south_bldg = map.bldg("south").id;
        let north_parking = map.parking_lane("north", 23).id;
        let south_parking = map.parking_lane("south", 23).id;

        let (spot, car) =
            h.seed_parked_cars(&mut sim, &mut rng, south_parking, Some(south_bldg), vec![2])[0];
        // Fill up some of the first spots, forcing parking to happen at spot 4
        h.seed_parked_cars(&mut sim, &mut rng, north_parking, None, (0..4).collect());
        h.seed_parked_cars(&mut sim, &mut rng, north_parking, None, (5..10).collect());
        sim.schedule_trip(
            Duration::ZERO,
            TripSpec::UsingParkedCar(
                SidewalkSpot::building(south_bldg, &map),
                spot,
                DrivingGoal::ParkNear(north_bldg),
            ),
            &map,
        );
        sim.spawn_all_trips(&map, &mut Timer::throwaway());
        h.setup_done(&sim);

        sim.run_until_expectations_met(
            &map,
            vec![Event::CarReachedParkingSpot(
                car,
                ParkingSpot::new(north_parking, 4),
            )],
            Duration::minutes(2),
        );
        sim.just_run_until_done(&map, Some(Duration::minutes(4)));
    });

    t.run_slow("wander_around_for_parking", |h| {
        let (map, mut sim, mut rng) =
            SimFlags::synthetic_test("parking_test", "wander_around_for_parking")
                .load(None, &mut Timer::throwaway());
        let north_bldg = map.bldg("north").id;
        let south_bldg = map.bldg("south").id;
        let north_parking = map.parking_lane("north", 23).id;
        let south_parking = map.parking_lane("south", 23).id;

        let (spot, car) =
            h.seed_parked_cars(&mut sim, &mut rng, south_parking, Some(south_bldg), vec![2])[0];
        // Fill up all of the north spots, forcing parking to happen on the south lane behind
        // the original spot
        h.seed_parked_cars(&mut sim, &mut rng, north_parking, None, (0..23).collect());
        sim.schedule_trip(
            Duration::ZERO,
            TripSpec::UsingParkedCar(
                SidewalkSpot::building(south_bldg, &map),
                spot,
                DrivingGoal::ParkNear(north_bldg),
            ),
            &map,
        );
        sim.spawn_all_trips(&map, &mut Timer::throwaway());
        h.setup_done(&sim);

        sim.run_until_expectations_met(
            &map,
            vec![Event::CarReachedParkingSpot(
                car,
                ParkingSpot::new(south_parking, 0),
            )],
            Duration::minutes(2),
        );
        sim.just_run_until_done(&map, Some(Duration::minutes(4)));
    });
}
