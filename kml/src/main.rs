extern crate abstutil;
extern crate geom;
extern crate kml;
extern crate structopt;

use geom::{GPSBounds, LonLat};
use structopt::StructOpt;

fn main() {
    let flags = kml::Flags::from_args();

    let mut timer = abstutil::Timer::new("extracting shapes from KML");

    // TODO don't hardcode
    let mut bounds = GPSBounds::new();
    bounds.update(LonLat::new(-122.4416, 47.5793));
    bounds.update(LonLat::new(-122.2421, 47.7155));

    let shapes = kml::load(&flags.input, &bounds, &mut timer).unwrap();

    println!("Writing to {}", flags.output);
    abstutil::write_binary(&flags.output, &shapes).unwrap();
    timer.done();
}
