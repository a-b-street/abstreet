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
            let bus = sim.seed_bus_route(route, &map)[0];
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
}

// TODO this test is strictly more complicated than bus_reaches_stops, should it subsume it?
/*#[test]
fn ped_uses_bus() {
    let (map, control_map, mut sim) = sim::load(
        sim::SimFlags::for_test("bus_reaches_stops"),
        Some(sim::Tick::from_seconds(30)),
        &mut abstutil::Timer::new("setup test"),
    );

    let route = map.get_bus_route("48").unwrap();
    let bus = sim.seed_bus_route(route, &map)[0];
    let ped_stop1 = route.stops[1];
    let ped_stop2 = route.stops[2];
    // TODO Need to fix this test after stabilizing a map
    let ped = sim.make_ped_using_bus(
        &map,
        map_model::BuildingID(123),
        map_model::BuildingID(456),
        sim::RouteID(0),
        ped_stop1,
        ped_stop2,
    );

    sim.run_until_expectations_met(
        &map,
        &control_map,
        vec![
            sim::Event::BusArrivedAtStop(bus, ped_stop1),
            sim::Event::PedEntersBus(ped, bus),
            sim::Event::BusDepartedFromStop(bus, ped_stop1),
            sim::Event::BusArrivedAtStop(bus, ped_stop2),
            sim::Event::PedLeavesBus(ped, bus),
            sim::Event::BusDepartedFromStop(bus, ped_stop2),
            sim::Event::BusArrivedAtStop(bus, route.stops[3]),
            // TODO PedReachedBuilding, once the seeding specifies a building instead of picking
        ],
        sim::Tick::from_minutes(10),
    );}*/
