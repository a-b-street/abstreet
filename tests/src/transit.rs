use abstutil::Timer;
use runner::TestRunner;
use sim;
use sim::{Event, SimFlags, Tick};

pub fn run(t: &mut TestRunner) {
    t.run_slow(
        "bus_reaches_stops",
        Box::new(|h| {
            let (map, control_map, mut sim) = sim::load(
                SimFlags::for_test("bus_reaches_stops"),
                Some(Tick::from_seconds(30)),
                &mut Timer::new("setup test"),
            );
            let route = map.get_bus_route("49").unwrap();
            let (_, buses) = sim.seed_bus_route(route, &map);
            let bus = buses[0];
            h.setup_done(&sim);

            let mut expectations: Vec<Event> = Vec::new();
            // TODO assert stuff about other buses as well, although the timing is a little unclear
            for stop in route.stops.iter().skip(1) {
                expectations.push(Event::BusArrivedAtStop(bus, *stop));
                expectations.push(Event::BusDepartedFromStop(bus, *stop));
            }

            sim.run_until_expectations_met(
                &map,
                &control_map,
                expectations,
                Tick::from_minutes(10),
            );
            sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
        }),
    );

    t.run_slow(
        "ped_uses_bus",
        Box::new(|h| {
            let (map, control_map, mut sim) = sim::load(
                SimFlags::for_test("ped_uses_bus"),
                Some(Tick::from_seconds(30)),
                &mut Timer::new("setup test"),
            );
            let route = map.get_bus_route("49").unwrap();
            let (route_id, buses) = sim.seed_bus_route(route, &map);
            let bus = buses[0];
            let ped_stop1 = route.stops[1];
            let ped_stop2 = route.stops[2];
            // TODO These should be buildings near the two stops. Programmatically find these?
            let start_bldg = map_model::BuildingID(1451);
            let goal_bldg = map_model::BuildingID(454);
            let ped = sim
                .seed_trip_using_bus(start_bldg, goal_bldg, route_id, ped_stop1, ped_stop2, &map);
            h.setup_done(&sim);

            sim.run_until_expectations_met(
                &map,
                &control_map,
                vec![
                    sim::Event::PedReachedBusStop(ped, ped_stop1),
                    sim::Event::BusArrivedAtStop(bus, ped_stop1),
                    sim::Event::PedEntersBus(ped, bus),
                    sim::Event::BusDepartedFromStop(bus, ped_stop1),
                    sim::Event::BusArrivedAtStop(bus, ped_stop2),
                    sim::Event::PedLeavesBus(ped, bus),
                    sim::Event::PedReachedBuilding(ped, goal_bldg),
                    sim::Event::BusDepartedFromStop(bus, ped_stop2),
                    sim::Event::BusArrivedAtStop(bus, route.stops[3]),
                ],
                sim::Tick::from_minutes(5),
            );
        }),
    );
}
