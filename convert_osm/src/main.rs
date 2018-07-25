extern crate abstutil;
extern crate convert_osm;
#[macro_use]
extern crate pretty_assertions;
extern crate structopt;

use convert_osm::{convert, Flags};
use structopt::StructOpt;

fn main() {
    let flags = Flags::from_args();
    let map = convert(&flags);
    println!("writing to {}", flags.output);
    abstutil::write_binary(&flags.output, &map).expect("serializing map failed");
}
