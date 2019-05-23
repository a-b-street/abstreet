use geom::GPSBounds;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "kml")]
struct Flags {
    /// KML file to read
    #[structopt(long = "input")]
    pub input: String,

    /// Output (serialized ExtraShapes) to write
    #[structopt(long = "output")]
    pub output: String,
}

fn main() {
    let flags = Flags::from_args();

    let shapes = kml::load(
        &flags.input,
        &GPSBounds::seattle_bounds(),
        &mut abstutil::Timer::new("extracting shapes from KML"),
    )
    .unwrap();

    println!("Writing to {}", flags.output);
    abstutil::write_binary(&flags.output, &shapes).unwrap();
}
