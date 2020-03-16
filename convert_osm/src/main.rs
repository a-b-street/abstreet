use abstutil::CmdArgs;
use convert_osm::{convert, Flags};

fn main() {
    let mut args = CmdArgs::new();
    let flags = Flags {
        osm: args.required("--osm"),
        parking_shapes: args.optional("--parking_shapes"),
        offstreet_parking: args.optional("--offstreet_parking"),
        sidewalks: args.optional("--sidewalks"),
        gtfs: args.optional("--gtfs"),
        neighborhoods: args.optional("--neighborhoods"),
        elevation: args.optional("--elevation"),
        clip: args.optional("--clip"),
        drive_on_right: args.true_false("--drive_on_right"),
        output: args.required("--output"),
    };
    args.done();

    let mut timer = abstutil::Timer::new(format!("generate {}", flags.output));
    let map = convert(&flags, &mut timer);
    timer.start("saving map");
    abstutil::write_binary(flags.output, &map);
    timer.stop("saving map");
}
