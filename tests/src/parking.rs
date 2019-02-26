use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use sim;

pub fn run(t: &mut TestRunner) {
    // TODO Lots of boilerplate between these two. Can we do better?

    t.run_slow("park_on_goal_st", |h| {
        let (map, mut sim) = sim::load(
            sim::SimFlags::synthetic_test("parking_test", "park_on_goal_st"),
            None,
            &mut Timer::throwaway(),
        );
        let north_bldg = map.bldg("north").id;
        let south_bldg = map.bldg("south").id;
        let north_parking = map.parking_lane("north", 23).id;
        let south_parking = map.parking_lane("south", 23).id;

        let car = sim.seed_specific_parked_cars(south_parking, south_bldg, vec![2])[0];
        // Fill up some of the first spots, forcing parking to happen at spot 4
        sim.seed_specific_parked_cars(north_parking, north_bldg, (0..4).collect());
        sim.seed_specific_parked_cars(north_parking, north_bldg, (5..10).collect());
        // TODO I just want to say (south_bldg, north_bldg), not mode...
        sim.seed_trip_using_parked_car(south_bldg, north_bldg, car, &map);
        h.setup_done(&sim);

        sim.run_until_expectations_met(
            &map,
            vec![sim::Event::CarReachedParkingSpot(
                car,
                sim::ParkingSpot::new(north_parking, 4),
            )],
            Duration::minutes(2),
        );
        sim.run_until_done(&map, |_| {}, Some(Duration::minutes(4)));
    });

    t.run_slow("wander_around_for_parking", |h| {
        let (map, mut sim) = sim::load(
            sim::SimFlags::synthetic_test("parking_test", "wander_around_for_parking"),
            None,
            &mut Timer::throwaway(),
        );
        let north_bldg = map.bldg("north").id;
        let south_bldg = map.bldg("south").id;
        let north_parking = map.parking_lane("north", 23).id;
        let south_parking = map.parking_lane("south", 23).id;

        let car = sim.seed_specific_parked_cars(south_parking, south_bldg, vec![2])[0];
        // Fill up all of the north spots, forcing parking to happen on the south lane behind
        // the original spot
        sim.seed_specific_parked_cars(north_parking, north_bldg, (0..23).collect());
        // TODO I just want to say (south_bldg, north_bldg), not mode...
        sim.seed_trip_using_parked_car(south_bldg, north_bldg, car, &map);
        h.setup_done(&sim);

        sim.run_until_expectations_met(
            &map,
            vec![sim::Event::CarReachedParkingSpot(
                car,
                sim::ParkingSpot::new(south_parking, 0),
            )],
            Duration::minutes(2),
        );
        sim.run_until_done(&map, |_| {}, Some(Duration::minutes(4)));
    });
}
