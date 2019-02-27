use geom::{GPSBounds, LonLat};
use structopt::StructOpt;

fn main() {
    let flags = kml::Flags::from_args();

    // TODO don't hardcode
    let mut bounds = GPSBounds::new();
    bounds.update(LonLat::new(-122.4416, 47.5793));
    bounds.update(LonLat::new(-122.2421, 47.7155));

    let shapes = kml::load(
        &flags.input,
        &bounds,
        &mut abstutil::Timer::new("extracting shapes from KML"),
    )
    .unwrap();

    println!("Writing to {}", flags.output);
    abstutil::write_binary(&flags.output, &shapes).unwrap();
}
