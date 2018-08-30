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

    let bus = sim.seed_bus(
        vec![
            map.get_l(map_model::LaneID(309)).bus_stops[0].clone(),
            map.get_l(map_model::LaneID(840)).bus_stops[0].clone(),
        ],
        &map,
    ).unwrap();
    // TODO kind of a hack, so that the sim isn't considered done until this long walk is complete
    sim.spawn_specific_pedestrian(&map, map_model::BuildingID(424), map_model::BuildingID(10));

    // TODO verify the expectations happen in order
    // TODO and print the time at which each expectation is true
    sim::init::run_until_done(
        &mut sim,
        &map,
        &control_map,
        vec![
            Box::new(move |sim| {
                sim.to_json()["transit_state"]["buses"][format!("{}", bus.0)]["AtStop"]
                    ["driving_lane"] == 840
            }),
            Box::new(move |sim| {
                sim.to_json()["transit_state"]["buses"][format!("{}", bus.0)]["AtStop"]
                    ["driving_lane"] == 309
            }),
        ],
    );
}
