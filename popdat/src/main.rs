fn main() {
    let mut timer = abstutil::Timer::new("creating popdat");
    let (trips, parcels) = popdat::psrc::import_trips(
        "../data/input/parcels_urbansim.txt",
        "../data/input/trips_2014.csv",
        &mut timer,
    )
    .unwrap();
    let popdat = popdat::PopDat { trips, parcels };
    abstutil::write_binary(abstutil::path_popdat(), &popdat);
}
