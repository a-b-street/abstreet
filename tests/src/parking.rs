use abstutil::Timer;
use runner::TestRunner;
use sim;

pub fn run(t: &mut TestRunner) {
    t.run_slow(
        "park_on_goal_st",
        Box::new(|h| {
            let (map, control_map, mut sim) = sim::load(
                sim::SimFlags::synthetic_test("parking_test", "park_on_goal_st"),
                None,
                &mut Timer::new("setup test"),
            );

            let north_bldg = map.bldg("north");
            let south_bldg = map.bldg("south");
            let north_parking = map.parking_lane("north", 18);
            let south_parking = map.parking_lane("south", 18);

            let car = sim.seed_specific_parked_cars(south_parking, south_bldg, vec![2])[0];
            // Fill up some of the first spots, forcing parking to happen at spot 4
            sim.seed_specific_parked_cars(north_parking, north_bldg, (0..4).collect());
            sim.seed_specific_parked_cars(north_parking, north_bldg, (5..10).collect());
            // TODO I just want to say (south_bldg, north_bldg), not mode...
            sim.seed_trip_using_parked_car(south_bldg, north_bldg, car, &map);
            h.setup_done(&sim);

            sim.run_until_expectations_met(
                &map,
                &control_map,
                vec![sim::Event::CarReachedParkingSpot(
                    car,
                    sim::ParkingSpot::new(north_parking, 4),
                )],
                sim::Tick::from_minutes(2),
            );
            sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
        }),
    );
}

/*
#[test]
fn wander_around_for_parking() {
    let (map, control_map, mut sim) = setup("wander_around_for_parking", make_test_map());
    let (south_parking, north_parking) = (LaneID(1), LaneID(4));
    let (north_bldg, south_bldg) = (BuildingID(0), BuildingID(1));

    assert_eq!(map.get_l(south_parking).number_parking_spots(), 8);
    assert_eq!(map.get_l(north_parking).number_parking_spots(), 8);
    // There's a free spot behind the car, so they have to loop around to their original lane to
    // find it.
    let car = sim.seed_specific_parked_cars(south_parking, south_bldg, (1..8).collect())[2];
    sim.seed_specific_parked_cars(north_parking, north_bldg, (0..8).collect());
    sim.make_ped_using_car(&map, car, north_bldg);

    sim.run_until_expectations_met(
        &map,
        &control_map,
        vec![sim::Event::CarReachedParkingSpot(
            car,
            sim::ParkingSpot::new(south_parking, 0),
        )],
        sim::Tick::from_minutes(2),
    );
    sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
}
*/
