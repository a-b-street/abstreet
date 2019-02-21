use abstutil::Timer;
use map_model::{Map, MapEdits};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "precompute")]
struct Flags {
    /// Map
    #[structopt(name = "load")]
    load: String,

    /// Name of map edits. Shouldn't be a full path or have the ".json"
    #[structopt(long = "edits_name")]
    edits_name: String,
}

fn main() {
    let flags = Flags::from_args();
    let mut timer = Timer::new(&format!(
        "precompute {} with {}",
        flags.load, flags.edits_name
    ));

    let edits: MapEdits = if flags.edits_name == "no_edits" {
        MapEdits::new(&flags.load)
    } else {
        abstutil::read_json(&format!(
            "../data/edits/{}/{}.json",
            flags.load, flags.edits_name
        ))
        .unwrap()
    };

    let raw_map_path = if flags.load.contains("synthetic") {
        let model: synthetic::Model =
            abstutil::read_json(&flags.load).expect(&format!("Couldn't load {}", &flags.load));
        model.export()
    } else {
        flags.load
    };

    let map = Map::new(&raw_map_path, edits, &mut timer).unwrap();
    timer.start("save map");
    map.save();
    timer.stop("save map");
    timer.done();
}
