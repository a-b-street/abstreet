use abstutil::Timer;
use map_model::Map;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "precompute")]
struct Flags {
    /// Map
    #[structopt(name = "load")]
    load: String,
}

fn main() {
    let flags = Flags::from_args();
    let mut timer = Timer::new(&format!("precompute {}", flags.load,));

    let raw_map_path = if flags.load.contains("synthetic") {
        let model: synthetic::Model =
            abstutil::read_json(&flags.load).expect(&format!("Couldn't load {}", &flags.load));
        model.export()
    } else {
        flags.load
    };

    let map = Map::new(&raw_map_path, &mut timer).unwrap();
    timer.start("save map");
    map.save();
    timer.stop("save map");
}
