use abstutil::CmdArgs;

fn main() {
    let mut args = CmdArgs::new();
    let cap_trips = args.optional_parse("--cap", |s| s.parse::<usize>());
    args.done();

    let mut timer = abstutil::Timer::new("creating popdat");
    let mut popdat = popdat::PopDat::import_all(&mut timer);

    let (trips, parcels) = popdat::psrc::import_trips(
        "../data/input/parcels_urbansim.txt",
        "../data/input/trips_2014.csv",
        &mut timer,
    )
    .unwrap();
    popdat.trips = trips;
    popdat.parcels = parcels;
    if let Some(n) = cap_trips {
        popdat.trips = popdat.trips.into_iter().take(n).collect();
    }

    abstutil::write_binary("../data/shapes/popdat.bin", &popdat).unwrap();
}
