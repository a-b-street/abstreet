mod contraction;
mod simplified;
mod walking;

use abstutil::Timer;
use geom::Distance;
use map_model::{LaneType, Map, PathRequest, Pathfinder, Position};
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

    /// Previously calculated CH to load
    #[structopt(long = "load_ch")]
    pub load_ch: Option<String>,

    /// Calculate a CH and save here
    #[structopt(long = "save_ch")]
    pub save_ch: Option<String>,
}

fn main() {
    let flags = Flags::from_args();
    let mut timer = Timer::new("benchmark pathfinding");
    let mut rng = XorShiftRng::from_seed([RNG_SEED; 16]);

    let map: Map = abstutil::read_binary(&flags.map, &mut timer).unwrap();
    println!(); // TODO Because Timer manages newlines poorly

    if let Some(path) = flags.save_ch {
        contraction::build_ch(path, &map, &mut timer);
        return;
    }

    let maybe_ch: Option<contraction::CHGraph> = if let Some(path) = flags.load_ch {
        Some(abstutil::read_binary(&path, &mut timer).unwrap())
    } else {
        None
    };

    let requests: Vec<PathRequest> = (0..NUM_PATHS)
        .map(|_| {
            let lane1 = loop {
                let l = map.all_lanes().choose(&mut rng).unwrap();
                if !l.is_parking() {
                    break l.id;
                }
            };
            let sidewalk = map.get_l(lane1).is_sidewalk();
            let lane2 = loop {
                let l = map.all_lanes().choose(&mut rng).unwrap();
                if sidewalk && l.is_sidewalk() {
                    break l.id;
                } else if !sidewalk && l.is_for_moving_vehicles() {
                    break l.id;
                }
            };
            PathRequest {
                start: Position::new(lane1, Distance::ZERO),
                end: Position::new(lane2, Distance::ZERO),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            }
        })
        .collect();

    if flags.enable_profiler {
        cpuprofiler::PROFILER
            .lock()
            .unwrap()
            .start("./profile")
            .unwrap();
    }

    if let Some(ref ch) = maybe_ch {
        timer.start_iter("compute paths using CH", requests.len());
        for req in &requests {
            timer.next();
            ch.pathfind(req);
        }
    }

    let car_graph = simplified::VehiclePathfinder::new(&map, vec![LaneType::Driving]);
    let bike_graph =
        simplified::VehiclePathfinder::new(&map, vec![LaneType::Driving, LaneType::Biking]);
    let bus_graph =
        simplified::VehiclePathfinder::new(&map, vec![LaneType::Driving, LaneType::Bus]);
    let walking_graph = walking::SidewalkPathfinder::new(&map, false);
    let walking_with_transit_graph = walking::SidewalkPathfinder::new(&map, true);

    timer.start_iter("compute paths using simplified approach", requests.len());
    for req in &requests {
        timer.next();
        if map.get_l(req.start.lane()).is_sidewalk() {
            walking_graph.pathfind(req, &map);
        } else {
            // TODO use bike or bus too, sometimes
            car_graph.pathfind(req, &map, &mut timer);
        }
    }

    timer.start_iter("compute paths using A*", requests.len());
    for req in requests {
        timer.next();
        Pathfinder::shortest_distance(&map, req);
    }

    if flags.enable_profiler {
        cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
    }
}
