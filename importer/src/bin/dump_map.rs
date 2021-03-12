use abstutil::{CmdArgs, Timer};
use map_model::Map;

fn main() {
    let mut args = CmdArgs::new();
    let map = Map::load_synchronously(args.required_free(), &mut Timer::throwaway());
    println!("{}", abstutil::to_json(&map));
    args.done();
}
