use abstutil::CmdArgs;
use convert_osm::{convert, Flags};

fn main() {
    let mut args = CmdArgs::new();
    let flags = Flags {
        osm: args.required("--osm"),
        parking_shapes: args.optional("--parking_shapes"),
        street_signs: args.optional("--street_signs"),
        offstreet_parking: args.optional("--offstreet_parking"),
        gtfs: args.optional("--gtfs"),
        neighborhoods: args.optional("--neighborhoods"),
        clip: args.optional("--clip"),
        output: args.required("--output"),
    };
    args.done();

    let mut timer = abstutil::Timer::new(&format!("generate {}", flags.output));
    let map = convert(&flags, &mut timer);
    println!("writing to {}", flags.output);
    timer.start("saving map");
    abstutil::write_binary(&flags.output, &map).expect("serializing map failed");
    timer.stop("saving map");
}
