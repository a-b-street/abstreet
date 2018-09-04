extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn bus_reaches_stops() {
    let (map, _, control_map, mut sim) = sim::load(
        "../data/small.abst".to_string(),
        "bus_reaches_stops".to_string(),
        Some(42),
        Some(sim::Tick::from_seconds(30)),
    );

    let stop1 = map.get_l(map_model::LaneID(309)).bus_stops[0].clone();
    let stop2 = map.get_l(map_model::LaneID(325)).bus_stops[0].clone();
    let stop3 = map.get_l(map_model::LaneID(840)).bus_stops[0].clone();
    let buses = sim.seed_bus_route(vec![stop1.clone(), stop2.clone(), stop3.clone()], &map);
    let (bus1, _, _) = (buses[0], buses[1], buses[2]);

    sim.run_until_expectations_met(
        &map,
        &control_map,
        // TODO assert stuff about other buses as well, although the timing is a little unclear
        vec![
            sim::Event::BusArrivedAtStop(bus1, stop2.clone()),
            sim::Event::BusDepartedFromStop(bus1, stop2),
            sim::Event::BusArrivedAtStop(bus1, stop3.clone()),
            sim::Event::BusDepartedFromStop(bus1, stop3),
            sim::Event::BusArrivedAtStop(bus1, stop1.clone()),
            sim::Event::BusDepartedFromStop(bus1, stop1),
        ],
        sim::Tick::from_minutes(10),
    );
    sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
}

// TODO this test is strictly more complicated than bus_reaches_stops, should it subsume it?
#[test]
fn ped_uses_bus() {
    let (map, _, control_map, mut sim) = sim::load(
        "../data/small.abst".to_string(),
        "bus_reaches_stops".to_string(),
        Some(42),
        Some(sim::Tick::from_seconds(30)),
    );

    let stop1 = map.get_l(map_model::LaneID(309)).bus_stops[0].clone();
    let stop2 = map.get_l(map_model::LaneID(325)).bus_stops[0].clone();
    let stop3 = map.get_l(map_model::LaneID(840)).bus_stops[0].clone();
    let buses = sim.seed_bus_route(vec![stop1.clone(), stop2.clone(), stop3.clone()], &map);
    let (bus, _, _) = (buses[0], buses[1], buses[2]);
    let ped = sim.make_ped_using_bus(
        &map,
        map_model::LaneID(550),
        map_model::LaneID(727),
        sim::RouteID(0),
        map.get_l(map_model::LaneID(325)).bus_stops[0].clone(),
        map.get_l(map_model::LaneID(840)).bus_stops[0].clone(),
    );

    sim.run_until_expectations_met(
        &map,
        &control_map,
        vec![
            sim::Event::BusArrivedAtStop(bus, stop2.clone()),
            sim::Event::PedEntersBus(ped, bus),
            sim::Event::BusDepartedFromStop(bus, stop2),
            sim::Event::BusArrivedAtStop(bus, stop3.clone()),
            sim::Event::PedLeavesBus(ped, bus),
            sim::Event::BusDepartedFromStop(bus, stop3),
            sim::Event::BusArrivedAtStop(bus, stop1.clone()),
            // TODO PedReachedBuilding, once the seeding specifies a building instead of picking
        ],
        sim::Tick::from_minutes(10),
    );
}
