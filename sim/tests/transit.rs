extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn bus_reaches_stops() {
    let (map, _, control_map, mut sim) = sim::init::load(
        "../data/small.abst".to_string(),
        "bus_reaches_stops".to_string(),
        Some(42),
        Some(sim::Tick::from_seconds(30)),
    );

    let stop1 = map.get_l(map_model::LaneID(309)).bus_stops[0].clone();
    let stop2 = map.get_l(map_model::LaneID(840)).bus_stops[0].clone();
    let bus = sim.seed_bus(vec![stop1.clone(), stop2.clone()], &map)
        .unwrap();

    sim::init::run_until_expectations_met(
        &mut sim,
        &map,
        &control_map,
        vec![
            sim::Event::BusArrivedAtStop(bus, stop2.clone()),
            sim::Event::BusDepartedFromStop(bus, stop2),
            sim::Event::BusArrivedAtStop(bus, stop1.clone()),
            sim::Event::BusDepartedFromStop(bus, stop1),
        ],
        sim::Tick::from_minutes(10),
    );
}
