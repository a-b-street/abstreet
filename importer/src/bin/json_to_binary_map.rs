use abstutil::{CmdArgs, Timer};
use map_model::Map;

fn main() {
    let mut args = CmdArgs::new();
    // TODO This can't handle the output of dump_map! What?!
    let mut map: Map = abstutil::read_json(args.required("--input"), &mut Timer::throwaway());
    map.map_loaded_directly();
    abstutil::write_binary(args.required("--output"), &map);
    args.done();
}
