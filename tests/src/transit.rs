use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use sim::{Event, Scenario, SidewalkSpot, SimFlags, TripSpec};

pub fn run(t: &mut TestRunner) {
    t.run_slow("bus_reaches_stops", |h| {
        let (map, mut sim, _) = SimFlags::for_test("bus_reaches_stops")
            .load(Some(Duration::seconds(30.0)), &mut Timer::throwaway());
        let route = map.get_bus_route("49").unwrap();
        let buses = sim.seed_bus_route(route, &map, &mut Timer::throwaway());
        let bus = buses[0];
        h.setup_done(&sim);

        let mut expectations: Vec<Event> = Vec::new();
        // TODO assert stuff about other buses as well, although the timing is a little unclear
        for stop in route.stops.iter().skip(1) {
            expectations.push(Event::BusArrivedAtStop(bus, *stop));
            expectations.push(Event::BusDepartedFromStop(bus, *stop));
        }

        sim.run_until_expectations_met(&map, expectations, Duration::minutes(10));
        // Make sure buses don't block a sim from being considered done
        sim.just_run_until_done(&map, Some(Duration::minutes(11)));
    });

    t.run_slow("ped_uses_bus", |h| {
        let (map, mut sim, mut rng) = SimFlags::for_test("ped_uses_bus")
            .load(Some(Duration::seconds(30.0)), &mut Timer::throwaway());
        let route = map.get_bus_route("49").unwrap();
        let buses = sim.seed_bus_route(route, &map, &mut Timer::throwaway());
        let bus = buses[0];
        let ped_stop1 = route.stops[1];
        let ped_stop2 = route.stops[2];
        // TODO These should be buildings near the two stops. Programmatically find these?
        let start_bldg = *map
            .get_l(map.get_bs(ped_stop1).sidewalk_pos.lane())
            .building_paths
            .last()
            .unwrap();
        // TODO Goal should be on the opposite side of the road from the stop, but that's hard to
        // express right now. :\
        let goal_bldg = map
            .get_l(map.get_bs(ped_stop2).sidewalk_pos.lane())
            .building_paths[0];
        let ped = sim
            .schedule_trip(
                Duration::ZERO,
                TripSpec::UsingTransit {
                    start: SidewalkSpot::building(start_bldg, &map),
                    route: route.id,
                    stop1: ped_stop1,
                    stop2: ped_stop2,
                    goal: SidewalkSpot::building(goal_bldg, &map),
                    ped_speed: Scenario::rand_ped_speed(&mut rng),
                },
                &map,
            )
            .0
            .unwrap();
        sim.spawn_all_trips(&map, &mut Timer::throwaway(), false);
        h.setup_done(&sim);

        sim.run_until_expectations_met(
            &map,
            vec![
                Event::PedReachedBusStop(ped, ped_stop1),
                Event::BusArrivedAtStop(bus, ped_stop1),
                Event::PedEntersBus(ped, bus),
                Event::BusDepartedFromStop(bus, ped_stop1),
                Event::BusArrivedAtStop(bus, ped_stop2),
                Event::PedLeavesBus(ped, bus),
                Event::PedReachedBuilding(ped, goal_bldg),
                Event::BusDepartedFromStop(bus, ped_stop2),
                Event::BusArrivedAtStop(bus, route.stops[3]),
            ],
            Duration::minutes(9),
        );
    });
}
