extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn bus_reaches_stops() {
    let (map, _, control_map, mut sim) = sim::load(
        "../data/maps/small.abst".to_string(),
        "bus_reaches_stops".to_string(),
        Some(42),
        Some(sim::Tick::from_seconds(30)),
    );

    let route = map.get_bus_route("48").unwrap();
    let bus = sim.seed_bus_route(route, &map)[0];
    let mut expectations: Vec<sim::Event> = Vec::new();
    // TODO assert stuff about other buses as well, although the timing is a little unclear
    for stop in route.stops.iter().skip(1) {
        expectations.push(sim::Event::BusArrivedAtStop(bus, *stop));
        expectations.push(sim::Event::BusDepartedFromStop(bus, *stop));
    }

    sim.run_until_expectations_met(
        &map,
        &control_map,
        expectations,
        sim::Tick::from_minutes(10),
    );
    sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
}

// TODO this test is strictly more complicated than bus_reaches_stops, should it subsume it?
#[test]
fn ped_uses_bus() {
    let (map, _, control_map, mut sim) = sim::load(
        "../data/maps/small.abst".to_string(),
        "bus_reaches_stops".to_string(),
        Some(42),
        Some(sim::Tick::from_seconds(30)),
    );

    let route = map.get_bus_route("48").unwrap();
    let bus = sim.seed_bus_route(route, &map)[0];
    let ped_stop1 = route.stops[1];
    let ped_stop2 = route.stops[2];
    let ped = sim.make_ped_using_bus(
        &map,
        map_model::LaneID(283),
        map_model::LaneID(553),
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
    );
}
