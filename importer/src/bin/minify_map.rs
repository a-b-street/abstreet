//! Removes nonessential parts of a Map, for the bike network tool.

use abstutil::{CmdArgs, Timer};
use map_model::Map;

fn main() {
    let mut args = CmdArgs::new();
    let mut timer = Timer::new("minify map");
    let mut map = Map::load_synchronously(args.required_free(), &mut timer);
    args.done();

    map.minify(&mut timer);
    // This also changes the name, so this won't overwrite anything
    map.save();
}
