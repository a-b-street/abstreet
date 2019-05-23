fn main() {
    let mut popdat = popdat::PopDat::import_all(&mut abstutil::Timer::new("importing popdat"));

    // TODO Productionize this.
    let parcels = popdat::psrc::import_parcels(
        "/home/dabreegster/Downloads/psrc/2014/landuse/parcels_urbansim.txt",
    )
    .unwrap();
    popdat.trips =
        popdat::psrc::import_trips("/home/dabreegster/Downloads/psrc/trips_2014.csv", parcels)
            .unwrap();

    abstutil::write_binary("../data/shapes/popdat", &popdat).unwrap();
}
