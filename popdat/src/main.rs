fn main() {
    // TODO Productionize.
    let parcels = popdat::psrc::import_parcels(
        "/home/dabreegster/Downloads/psrc/2014/landuse/parcels_urbansim.txt",
    )
    .unwrap();
    println!("{} parcels", parcels.len());
    let trips =
        popdat::psrc::import_trips("/home/dabreegster/Downloads/psrc/trips_2014.csv", parcels)
            .unwrap();
    println!("{} trips", trips.len());

    let popdat = popdat::PopDat::import_all(&mut abstutil::Timer::new("importing popdat"));
    abstutil::write_binary("../data/shapes/popdat", &popdat).unwrap();
}
