use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use map_model::{BuildingID, IntersectionID};
use sim::{DrivingGoal, Event, Scenario, SidewalkSpot, SimFlags, TripSpec};

pub fn run(t: &mut TestRunner) {
    t.run_slow("bike_from_border", |h| {
        let (map, mut sim, mut rng) = SimFlags::for_test("bike_from_border")
            .load(Some(Duration::seconds(30.0)), &mut Timer::throwaway());
        // TODO Hardcoding IDs is fragile
        let goal_bldg = BuildingID(319);
        let (ped, bike) = sim.schedule_trip(
            Duration::ZERO,
            TripSpec::UsingBike(
                SidewalkSpot::start_at_border(IntersectionID(186), &map).unwrap(),
                Scenario::rand_bike(&mut rng),
                DrivingGoal::ParkNear(goal_bldg),
            ),
            &map,
        );
        sim.spawn_all_trips(&map, &mut Timer::throwaway());
        h.setup_done(&sim);

        sim.run_until_expectations_met(
            &map,
            vec![
                Event::BikeStoppedAtSidewalk(
                    bike.unwrap(),
                    map.get_b(goal_bldg).front_path.sidewalk.lane(),
                ),
                Event::PedReachedBuilding(ped.unwrap(), goal_bldg),
            ],
            Duration::minutes(3),
        );
        sim.run_until_done(&map, |_| {}, Some(Duration::minutes(4)));
    });
}
