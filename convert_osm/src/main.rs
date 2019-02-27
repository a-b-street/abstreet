use convert_osm::{convert, Flags};
use structopt::StructOpt;

fn main() {
    let flags = Flags::from_args();
    let mut timer = abstutil::Timer::new(&format!("generate {}", flags.output));
    let map = convert(&flags, &mut timer);
    println!("writing to {}", flags.output);
    timer.start("saving map");
    abstutil::write_binary(&flags.output, &map).expect("serializing map failed");
    timer.stop("saving map");
}
