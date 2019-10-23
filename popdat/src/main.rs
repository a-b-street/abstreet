fn main() {
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
    abstutil::write_binary("../data/shapes/popdat.bin", &popdat).unwrap();
}
