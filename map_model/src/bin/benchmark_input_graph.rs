//! A simple tool that just runs a simulation for the specified number of hours. Use for profiling
//! and benchmarking.

fn main() {
    let mut timer = abstutil::Timer::new("benchmark make_input_graph");
    let mut args = abstutil::CmdArgs::new();
    let map = map_model::Map::load_synchronously(args.required_free(), &mut timer);
    let iters: usize = args.optional_parse("--iters", |x| x.parse()).unwrap_or(10);
    args.done();

    let mut params = map_model::RoutingParams::default();
    params.avoid_steep_incline_penalty = 1.1;

    let req = map_model::PathRequest::between_buildings(
        &map,
        map_model::BuildingID(0),
        map_model::BuildingID(1),
        map_model::PathConstraints::Bike,
    )
    .unwrap();
    timer.start_iter("pathfind_with_params", iters);
    for _ in 0..iters {
        timer.next();
        let _ = map.pathfind_with_params(req.clone(), &params);
    }
}
