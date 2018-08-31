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
    let stop2 = map.get_l(map_model::LaneID(840)).bus_stops[0].clone();
    let buses = sim.seed_bus_route(vec![stop1.clone(), stop2.clone()], &map);
    let (bus1, bus2) = (buses[0], buses[1]);

    sim.run_until_expectations_met(
        &map,
        &control_map,
        // TODO assert stuff about bus2 as well, although the timing is a little unclear
        vec![
            sim::Event::BusArrivedAtStop(bus1, stop2.clone()),
            sim::Event::BusDepartedFromStop(bus1, stop2),
            sim::Event::BusArrivedAtStop(bus1, stop1.clone()),
            sim::Event::BusDepartedFromStop(bus1, stop1),
        ],
        sim::Tick::from_minutes(10),
    );
}
