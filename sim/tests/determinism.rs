extern crate control;
extern crate geom;
extern crate map_model;
extern crate sim;

#[test]
fn from_scratch() {
    // This assumes this map has been built
    let input = "../data/small.abst";
    let rng_seed = 42;
    let spawn_count = 1000;

    println!("Creating two simulations");
    let data = map_model::load_pb(input).expect("Couldn't load input");
    let map = map_model::Map::new(&data);
    let geom_map = geom::GeomMap::new(&map);
    let control_map = control::ControlMap::new(&map, &geom_map);

    let mut sim1 = sim::straw_model::Sim::new(&map, &geom_map, Some(rng_seed));
    let mut sim2 = sim::straw_model::Sim::new(&map, &geom_map, Some(rng_seed));
    sim1.spawn_many_on_empty_roads(spawn_count);
    sim2.spawn_many_on_empty_roads(spawn_count);

    for _ in 1..1200 {
        if sim1 != sim2 {
            // TODO need to sort dicts in json output to compare
            sim1.write_savestate("sim1_state.json").unwrap();
            sim2.write_savestate("sim2_state.json").unwrap();
            panic!("sim state differs at {}. compare sim1_state.json and sim2_state.json", sim1.time);
        }
        sim1.step(&geom_map, &map, &control_map);
        sim2.step(&geom_map, &map, &control_map);
    }
}
