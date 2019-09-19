use abstutil::CmdArgs;

fn main() {
    let mut args = CmdArgs::new();
    let cap_trips = args.optional_parse("--cap", |s| s.parse::<usize>());
    args.done();

    let mut timer = abstutil::Timer::new("creating popdat");
    let mut popdat = popdat::PopDat::import_all(&mut timer);

    // TODO Productionize this.
    // https://file.ac/cLdO7Hp_OB0/ has trips_2014.csv. https://file.ac/Xdjmi8lb2dA/ has the 2014
    // inputs.
    let (trips, parcels) = popdat::psrc::import_trips(
        "/home/dabreegster/Downloads/psrc/2014/landuse/parcels_urbansim.txt",
        "/home/dabreegster/Downloads/psrc/trips_2014.csv",
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
