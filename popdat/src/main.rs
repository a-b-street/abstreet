fn main() {
    let mut timer = abstutil::Timer::new("creating popdat");
    let mut popdat = popdat::PopDat::import_all(&mut timer);

    // TODO Productionize this.
    // https://file.ac/cLdO7Hp_OB0/ has trips_2014.csv. https://file.ac/Xdjmi8lb2dA/ has the 2014
    // inputs.
    popdat.trips = popdat::psrc::import_trips(
        "/home/dabreegster/Downloads/psrc/2014/landuse/parcels_urbansim.txt",
        "/home/dabreegster/Downloads/psrc/trips_2014.csv",
        &mut timer,
    )
    .unwrap();

    abstutil::write_binary("../data/shapes/popdat", &popdat).unwrap();
}
