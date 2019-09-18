use abstutil::CmdArgs;
use geom::GPSBounds;

fn main() {
    let mut args = CmdArgs::new();
    let input = args.required("--input");
    let output = args.required("--output");
    args.done();

    let shapes = kml::load(
        &input,
        &GPSBounds::seattle_bounds(),
        &mut abstutil::Timer::new("extracting shapes from KML"),
    )
    .unwrap();

    println!("Writing to {}", output);
    abstutil::write_binary(&output, &shapes).unwrap();
}
