use abstutil::Timer;
use geom::Distance;
use map_model::{Map, PathRequest, Pathfinder, Position};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use structopt::StructOpt;

const RNG_SEED: u8 = 42;
const NUM_PATHS: usize = 150;

#[derive(StructOpt, Debug)]
#[structopt(name = "benchmark_pathfinding")]
struct Flags {
    /// Map to load
    #[structopt(name = "map")]
    pub map: String,

    /// Enable cpuprofiler?
    #[structopt(long = "enable_profiler")]
    pub enable_profiler: bool,
}

fn main() {
    let flags = Flags::from_args();
    let mut timer = Timer::new("benchmark pathfinding");
    let mut rng = XorShiftRng::from_seed([RNG_SEED; 16]);

    let map: Map = abstutil::read_binary(&flags.map, &mut timer).unwrap();

    if flags.enable_profiler {
        cpuprofiler::PROFILER
            .lock()
            .unwrap()
            .start("./profile")
            .unwrap();
    }
    println!(); // TODO Because Timer manages newlines poorly
    timer.start_iter("compute paths", NUM_PATHS);
    for _ in 0..NUM_PATHS {
        timer.next();
        let lane1 = map.all_lanes().choose(&mut rng).unwrap().id;
        let lane2 = map.all_lanes().choose(&mut rng).unwrap().id;
        Pathfinder::shortest_distance(
            &map,
            PathRequest {
                start: Position::new(lane1, Distance::ZERO),
                end: Position::new(lane2, Distance::ZERO),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
        );
    }
    if flags.enable_profiler {
        cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
    }
}
